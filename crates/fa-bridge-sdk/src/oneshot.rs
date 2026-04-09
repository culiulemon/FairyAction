use crate::context::{ActionContext, RunMode};
use crate::types::App;

pub fn parse_cli_args(args: &[String]) -> serde_json::Map<String, serde_json::Value> {
    let mut params = serde_json::Map::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with("--") {
            let key = arg.trim_start_matches("--").to_string();
            if i + 1 < args.len() {
                let val = &args[i + 1];
                if let Ok(n) = val.parse::<i64>() {
                    params.insert(key, serde_json::Value::Number(n.into()));
                } else if val == "true" {
                    params.insert(key, serde_json::Value::Bool(true));
                } else if val == "false" {
                    params.insert(key, serde_json::Value::Bool(false));
                } else {
                    params.insert(key, serde_json::Value::String(val.clone()));
                }
                i += 2;
            } else {
                params.insert(key, serde_json::Value::Bool(true));
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    params
}

impl App {
    pub fn run_oneshot(&self, args: &[String]) {
        if args.is_empty() {
            eprintln!("Usage: <binary> <action> [--param value ...]");
            std::process::exit(1);
        }

        let action_name = &args[0];
        let params = parse_cli_args(&args[1..]);
        let params_value = serde_json::Value::Object(params);

        for domain in &self.domains {
            for action in &domain.actions {
                if action.name == *action_name {
                    let ctx = ActionContext {
                        domain: domain.name.clone(),
                        action: action.name.clone(),
                        mode: RunMode::Oneshot,
                    };
                    match (action.handler)(params_value, &ctx) {
                        Ok(result) => {
                            println!("{}", serde_json::to_string_pretty(&result).unwrap());
                            std::process::exit(0);
                        }
                        Err(e) => {
                            let err_msg = serde_json::json!({
                                "error": format!("{}", e)
                            });
                            eprintln!(
                                "{}",
                                serde_json::to_string_pretty(&err_msg).unwrap()
                            );
                            std::process::exit(1);
                        }
                    }
                }
            }
        }

        eprintln!("Unknown action: {}", action_name);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_args() {
        let args: Vec<String> = vec![];
        let params = parse_cli_args(&args);
        assert!(params.is_empty());
    }

    #[test]
    fn test_parse_string_param() {
        let args = vec!["--name".to_string(), "hello".to_string()];
        let params = parse_cli_args(&args);
        assert_eq!(params.get("name").unwrap().as_str().unwrap(), "hello");
    }

    #[test]
    fn test_parse_int_param() {
        let args = vec!["--count".to_string(), "42".to_string()];
        let params = parse_cli_args(&args);
        assert_eq!(params.get("count").unwrap().as_i64().unwrap(), 42);
    }

    #[test]
    fn test_parse_bool_true_param() {
        let args = vec!["--verbose".to_string(), "true".to_string()];
        let params = parse_cli_args(&args);
        assert_eq!(params.get("verbose").unwrap().as_bool().unwrap(), true);
    }

    #[test]
    fn test_parse_bool_false_param() {
        let args = vec!["--verbose".to_string(), "false".to_string()];
        let params = parse_cli_args(&args);
        assert_eq!(params.get("verbose").unwrap().as_bool().unwrap(), false);
    }

    #[test]
    fn test_parse_flag_without_value() {
        let args = vec!["--debug".to_string()];
        let params = parse_cli_args(&args);
        assert_eq!(params.get("debug").unwrap().as_bool().unwrap(), true);
    }

    #[test]
    fn test_parse_multiple_params() {
        let args = vec![
            "--name".to_string(),
            "test".to_string(),
            "--count".to_string(),
            "10".to_string(),
            "--verbose".to_string(),
            "true".to_string(),
        ];
        let params = parse_cli_args(&args);
        assert_eq!(params.get("name").unwrap().as_str().unwrap(), "test");
        assert_eq!(params.get("count").unwrap().as_i64().unwrap(), 10);
        assert_eq!(params.get("verbose").unwrap().as_bool().unwrap(), true);
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_parse_non_flag_args_ignored() {
        let args = vec![
            "action_name".to_string(),
            "--key".to_string(),
            "value".to_string(),
        ];
        let params = parse_cli_args(&args);
        assert!(params.get("action_name").is_none());
        assert_eq!(params.get("key").unwrap().as_str().unwrap(), "value");
    }

    #[test]
    fn test_parse_negative_int() {
        let args = vec!["--offset".to_string(), "-5".to_string()];
        let params = parse_cli_args(&args);
        assert_eq!(params.get("offset").unwrap().as_i64().unwrap(), -5);
    }
}
