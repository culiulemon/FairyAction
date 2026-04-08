use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

#[derive(Debug)]
pub struct ProcessResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("process timed out after {0}s")]
    Timeout(u32),
    #[error("failed to spawn process: {0}")]
    SpawnFailed(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub async fn execute_process(
    binary: &Path,
    args: &[String],
    env: Option<&HashMap<String, String>>,
    working_dir: Option<&Path>,
    timeout: Option<u32>,
) -> Result<ProcessResult, ProcessError> {
    let mut cmd = tokio::process::Command::new(binary);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(env_vars) = env {
        for (key, value) in env_vars {
            cmd.env(key, value);
        }
    }

    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }

    let child = cmd.spawn().map_err(|e| ProcessError::SpawnFailed(e.to_string()))?;

    let result = if let Some(secs) = timeout {
        match tokio::time::timeout(
            std::time::Duration::from_secs(secs as u64),
            child.wait_with_output(),
        )
        .await
        {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => return Err(ProcessError::Io(e)),
            Err(_) => {
                return Err(ProcessError::Timeout(secs));
            }
        }
    } else {
        child.wait_with_output().await?
    };

    let exit_code = result.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
    let stderr = String::from_utf8_lossy(&result.stderr).to_string();

    Ok(ProcessResult {
        exit_code,
        stdout,
        stderr,
    })
}
