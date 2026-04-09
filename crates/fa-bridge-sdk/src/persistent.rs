use std::io::{BufRead, Write};

use crate::context::{ActionContext, RunMode};
use crate::types::App;

impl App {
    pub fn run_persistent(&self) {
        let caps = self.build_capabilities_json();
        let hello_msg = format!(
            "hello\x1F{}\x1F{}\x1F{}\n",
            self.name,
            self.version,
            serde_json::to_string(&caps).unwrap()
        );
        std::io::stdout().write_all(hello_msg.as_bytes()).unwrap();
        std::io::stdout().flush().unwrap();

        let stdin = std::io::stdin();
        let mut reader = std::io::BufReader::new(stdin.lock());

        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }

            let line = line.trim_end_matches('\n').trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }

            if line == "shutdown" {
                println!("bye");
                std::io::stdout().flush().unwrap();
                break;
            }

            let parts: Vec<&str> = line.split('\x1F').collect();
            if parts.is_empty() {
                continue;
            }

            let msg_type = parts[0];
            match msg_type {
                "call" => {
                    let domain_name = parts.get(1).unwrap_or(&"");
                    let action_name = parts.get(2).unwrap_or(&"");
                    let payload_str = parts.get(3).unwrap_or(&"{}");
                    let payload: serde_json::Value =
                        serde_json::from_str(payload_str).unwrap_or(serde_json::Value::Null);

                    let mut found = false;
                    for domain in &self.domains {
                        if domain.name != *domain_name {
                            continue;
                        }
                        for action in &domain.actions {
                            if action.name != *action_name {
                                continue;
                            }
                            found = true;

                            let ctx = ActionContext {
                                domain: domain.name.clone(),
                                action: action.name.clone(),
                                mode: RunMode::Persistent,
                            };

                            match (action.handler)(payload.clone(), &ctx) {
                                Ok(result) => {
                                    let resp = format!(
                                        "ok\x1F{}\x1F{}\x1F{}\n",
                                        domain.name,
                                        action.name,
                                        serde_json::to_string(&result).unwrap()
                                    );
                                    std::io::stdout()
                                        .write_all(resp.as_bytes())
                                        .unwrap();
                                    std::io::stdout().flush().unwrap();
                                }
                                Err(e) => {
                                    let err_obj = serde_json::json!({
                                        "错误码": "HANDLER_ERROR",
                                        "错误信息": format!("{}", e)
                                    });
                                    let resp = format!(
                                        "error\x1F{}\x1F{}\x1F{}\n",
                                        domain.name,
                                        action.name,
                                        serde_json::to_string(&err_obj).unwrap()
                                    );
                                    std::io::stdout()
                                        .write_all(resp.as_bytes())
                                        .unwrap();
                                    std::io::stdout().flush().unwrap();
                                }
                            }
                            break;
                        }
                        if found {
                            break;
                        }
                    }

                    if !found {
                        let resp = format!(
                            "error\x1F{}\x1F{}\x1F{}\n",
                            domain_name,
                            action_name,
                            serde_json::to_string(&serde_json::json!({
                                "错误码": "NOT_FOUND",
                                "错误信息": format!("action not found: {} / {}", domain_name, action_name)
                            }))
                            .unwrap()
                        );
                        std::io::stdout().write_all(resp.as_bytes()).unwrap();
                        std::io::stdout().flush().unwrap();
                    }
                }
                _ => {
                    let resp = format!(
                        "error\x1F\x1F\x1F{}\n",
                        serde_json::to_string(&serde_json::json!({
                            "错误码": "UNKNOWN_MESSAGE",
                            "错误信息": format!("unknown message type: {}", msg_type)
                        }))
                        .unwrap()
                    );
                    std::io::stdout().write_all(resp.as_bytes()).unwrap();
                    std::io::stdout().flush().unwrap();
                }
            }
        }
    }
}
