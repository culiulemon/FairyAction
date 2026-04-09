use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Semaphore;

#[allow(dead_code)]
struct ProcessEntry {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    package: String,
    started_at: Instant,
    last_active: Instant,
    semaphore: Arc<Semaphore>,
}

pub struct ProcessPool {
    processes: HashMap<String, ProcessEntry>,
    idle_timeout: Duration,
}

pub struct CallResult {
    pub success: bool,
    pub domain: String,
    pub action: String,
    pub payload: serde_json::Value,
}

impl ProcessPool {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            idle_timeout: Duration::from_secs(300),
        }
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            processes: HashMap::new(),
            idle_timeout: timeout,
        }
    }

    pub async fn get_or_spawn(
        &mut self,
        package: &str,
        binary: &PathBuf,
    ) -> anyhow::Result<()> {
        if let Some(entry) = self.processes.get_mut(package) {
            if entry.child.try_wait()?.is_none() {
                return Ok(());
            }
            self.processes.remove(package);
        }

        let mut child = Command::new(binary)
            .arg("--serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to spawn process for package '{package}'"))?;

        let stdin = child.stdin.take().context("failed to take stdin")?;
        let stdout = child.stdout.take().context("failed to take stdout")?;
        let mut stdout = BufReader::new(stdout);

        let mut hello_line = String::new();
        stdout
            .read_line(&mut hello_line)
            .await
            .context("failed to read hello handshake")?;

        let hello_line = hello_line.trim_end_matches('\n').trim_end_matches('\r');
        if !hello_line.starts_with("hello\x1F") {
            let _ = child.kill().await;
            bail!(
                "invalid hello handshake from package '{package}': {hello_line}"
            );
        }

        let now = Instant::now();
        self.processes.insert(
            package.to_string(),
            ProcessEntry {
                child,
                stdin,
                stdout,
                package: package.to_string(),
                started_at: now,
                last_active: now,
                semaphore: Arc::new(Semaphore::new(1)),
            },
        );

        Ok(())
    }

    pub async fn send_call(
        &mut self,
        package: &str,
        domain: &str,
        action: &str,
        payload: &serde_json::Value,
    ) -> anyhow::Result<CallResult> {
        let entry = self
            .processes
            .get_mut(package)
            .with_context(|| format!("no active process for package '{package}'"))?;

        if entry.child.try_wait()?.is_some() {
            self.processes.remove(package);
            bail!("process for package '{package}' has exited");
        }

        let permit = entry.semaphore.clone().acquire_owned().await?;

        let entry = self.processes.get_mut(package).with_context(|| {
            format!("process for package '{package}' was removed during semaphore wait")
        })?;

        let payload_json = serde_json::to_string(payload)?;
        let request = format!("call\x1F{domain}\x1F{action}\x1F{payload_json}\n");
        entry.stdin.write_all(request.as_bytes()).await?;
        entry.stdin.flush().await?;

        let mut response_line = String::new();
        entry.stdout.read_line(&mut response_line).await?;
        let response_line = response_line
            .trim_end_matches('\n')
            .trim_end_matches('\r');

        entry.last_active = Instant::now();
        drop(permit);

        if let Some(rest) = response_line.strip_prefix("ok\x1F") {
            let parts: Vec<&str> = rest.splitn(3, '\x1F').collect();
            if parts.len() != 3 {
                bail!("malformed ok response for package '{package}'");
            }
            let resp_domain = parts[0].to_string();
            let resp_action = parts[1].to_string();
            let resp_payload: serde_json::Value =
                serde_json::from_str(parts[2]).with_context(|| {
                    format!("failed to parse response payload for package '{package}'")
                })?;
            Ok(CallResult {
                success: true,
                domain: resp_domain,
                action: resp_action,
                payload: resp_payload,
            })
        } else if let Some(rest) = response_line.strip_prefix("error\x1F") {
            Ok(CallResult {
                success: false,
                domain: domain.to_string(),
                action: action.to_string(),
                payload: serde_json::Value::String(rest.to_string()),
            })
        } else {
            bail!(
                "unexpected response from package '{package}': {response_line}"
            );
        }
    }

    pub async fn shutdown(&mut self, package: &str) -> anyhow::Result<()> {
        let Some(entry) = self.processes.get_mut(package) else {
            return Ok(());
        };

        let _ = entry.stdin.write_all(b"shutdown\n").await;
        let _ = entry.stdin.flush().await;

        let mut bye_line = String::new();
        let _ = entry.stdout.read_line(&mut bye_line).await;

        let mut entry = self.processes.remove(package).unwrap();
        if entry.child.try_wait()?.is_some() {
            let _ = entry.child.kill().await;
        }

        Ok(())
    }

    pub async fn shutdown_all(&mut self) -> anyhow::Result<()> {
        let packages: Vec<String> = self.processes.keys().cloned().collect();
        for package in packages {
            self.shutdown(&package).await?;
        }
        Ok(())
    }

    pub fn is_alive(&mut self, package: &str) -> bool {
        self.processes
            .get_mut(package)
            .is_some_and(|e| e.child.try_wait().ok().flatten().is_none())
    }

    pub async fn cleanup_idle(&mut self) {
        let packages: Vec<String> = self
            .processes
            .iter()
            .filter(|(_, entry)| entry.last_active.elapsed() > self.idle_timeout)
            .map(|(k, _)| k.clone())
            .collect();

        for package in packages {
            let _ = self.shutdown(&package).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_empty_pool() {
        let pool = ProcessPool::new();
        assert!(pool.processes.is_empty());
        assert_eq!(pool.idle_timeout, Duration::from_secs(300));
    }

    #[test]
    fn test_with_timeout_custom_duration() {
        let pool = ProcessPool::with_timeout(Duration::from_secs(60));
        assert!(pool.processes.is_empty());
        assert_eq!(pool.idle_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_is_alive_nonexistent_package() {
        let mut pool = ProcessPool::new();
        assert!(!pool.is_alive("nonexistent"));
    }

    #[tokio::test]
    async fn test_cleanup_idle_empty_pool() {
        let mut pool = ProcessPool::new();
        pool.cleanup_idle().await;
        assert!(pool.processes.is_empty());
    }
}
