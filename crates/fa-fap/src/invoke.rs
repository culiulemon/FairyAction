use crate::manifest::InvokeConfig;

#[derive(Debug, thiserror::Error)]
pub enum InvokeError {
    #[error("missing parameter: {0}")]
    MissingParam(String),
    #[error("host data directory not configured")]
    HostDataDirNotConfigured,
    #[error("invalid parameter type for '{name}': {detail}")]
    InvalidParamType { name: String, detail: String },
}

pub struct SpecialVars {
    pub temp_dir: String,
    pub package_dir: String,
    pub host_data_dir: Option<String>,
}

pub fn render_invoke(
    invoke: &InvokeConfig,
    params: &serde_json::Value,
    special_vars: &SpecialVars,
) -> Result<Vec<String>, InvokeError> {
    let mut result = Vec::new();

    for template in &invoke.args {
        let expanded = render_template(template, params, special_vars)?;
        result.extend(expanded);
    }

    Ok(result)
}

fn render_template(
    template: &str,
    params: &serde_json::Value,
    special_vars: &SpecialVars,
) -> Result<Vec<String>, InvokeError> {
    let mut result_args = vec![String::new()];
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' && chars.peek() == Some(&'{') {
            chars.next();
            let mut var_name = String::new();
            let mut depth = 1;
            loop {
                match chars.next() {
                    Some('{') if chars.peek() == Some(&'{') => {
                        depth += 1;
                        var_name.push('{');
                        var_name.push('{');
                        chars.next();
                    }
                    Some('}') if chars.peek() == Some(&'}') => {
                        depth -= 1;
                        if depth == 0 {
                            chars.next();
                            break;
                        }
                        var_name.push('}');
                        var_name.push('}');
                        chars.next();
                    }
                    Some(c) => var_name.push(c),
                    None => break,
                }
            }

            let replacement = resolve_variable(&var_name, params, special_vars)?;
            match replacement {
                ResolvedValue::Single(s) => {
                    for arg in &mut result_args {
                        arg.push_str(&s);
                    }
                }
                ResolvedValue::Multiple(items) => {
                    if items.is_empty() {
                        continue;
                    }
                    let last_arg = result_args.pop().unwrap();
                    let mut expanded: Vec<String> = Vec::new();
                    for (i, item) in items.iter().enumerate() {
                        if i == 0 {
                            expanded.push(format!("{last_arg}{item}"));
                        } else {
                            expanded.push(item.clone());
                        }
                    }
                    result_args.extend(expanded);
                }
            }
        } else {
            for arg in &mut result_args {
                arg.push(ch);
            }
        }
    }

    Ok(result_args)
}

enum ResolvedValue {
    Single(String),
    Multiple(Vec<String>),
}

fn resolve_variable(
    name: &str,
    params: &serde_json::Value,
    special_vars: &SpecialVars,
) -> Result<ResolvedValue, InvokeError> {
    match name {
        "临时目录" => Ok(ResolvedValue::Single(special_vars.temp_dir.clone())),
        "包目录" => Ok(ResolvedValue::Single(special_vars.package_dir.clone())),
        "宿主数据目录" => {
            special_vars
                .host_data_dir
                .clone()
                .map(ResolvedValue::Single)
                .ok_or(InvokeError::HostDataDirNotConfigured)
        }
        "来源路径的父目录" => {
            let parent = std::path::Path::new(&special_vars.package_dir)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| special_vars.package_dir.clone());
            Ok(ResolvedValue::Single(parent))
        }
        _ => resolve_from_params(name, params),
    }
}

fn resolve_from_params(
    name: &str,
    params: &serde_json::Value,
) -> Result<ResolvedValue, InvokeError> {
    let value = params
        .get(name)
        .ok_or_else(|| InvokeError::MissingParam(name.to_string()))?;

    match value {
        serde_json::Value::String(s) => Ok(ResolvedValue::Single(s.clone())),
        serde_json::Value::Number(n) => Ok(ResolvedValue::Single(n.to_string())),
        serde_json::Value::Bool(b) => Ok(ResolvedValue::Single(b.to_string())),
        serde_json::Value::Array(arr) => {
            let items: Result<Vec<String>, InvokeError> = arr
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => Ok(s.clone()),
                    serde_json::Value::Number(n) => Ok(n.to_string()),
                    serde_json::Value::Bool(b) => Ok(b.to_string()),
                    serde_json::Value::Null => Err(InvokeError::InvalidParamType {
                        name: name.to_string(),
                        detail: "null value in array".to_string(),
                    }),
                    _ => Err(InvokeError::InvalidParamType {
                        name: name.to_string(),
                        detail: format!("unsupported array element type: {v}"),
                    }),
                })
                .collect();
            Ok(ResolvedValue::Multiple(items?))
        }
        serde_json::Value::Null => Err(InvokeError::InvalidParamType {
            name: name.to_string(),
            detail: "null value".to_string(),
        }),
        _ => Err(InvokeError::InvalidParamType {
            name: name.to_string(),
            detail: format!("unsupported type: {value}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_invoke(args: Vec<&str>) -> InvokeConfig {
        InvokeConfig {
            args: args.into_iter().map(String::from).collect(),
            env: None,
            exit_code: None,
            output: None,
            timeout: None,
        }
    }

    fn make_special_vars() -> SpecialVars {
        SpecialVars {
            temp_dir: "/tmp/fap".to_string(),
            package_dir: "/opt/fap/com.example.demo".to_string(),
            host_data_dir: Some("/data/host".to_string()),
        }
    }

    #[test]
    fn basic_param_replacement() {
        let invoke = make_invoke(vec!["echo", "{{message}}"]);
        let params = serde_json::json!({"message": "hello world"});
        let result = render_invoke(&invoke, &params, &make_special_vars()).unwrap();
        assert_eq!(result, vec!["echo", "hello world"]);
    }

    #[test]
    fn integer_param() {
        let invoke = make_invoke(vec!["--count", "{{count}}"]);
        let params = serde_json::json!({"count": 42});
        let result = render_invoke(&invoke, &params, &make_special_vars()).unwrap();
        assert_eq!(result, vec!["--count", "42"]);
    }

    #[test]
    fn bool_param() {
        let invoke = make_invoke(vec!["--flag", "{{flag}}"]);
        let params = serde_json::json!({"flag": true});
        let result = render_invoke(&invoke, &params, &make_special_vars()).unwrap();
        assert_eq!(result, vec!["--flag", "true"]);
    }

    #[test]
    fn array_param_expansion() {
        let invoke = make_invoke(vec!["run", "{{files}}"]);
        let params = serde_json::json!({"files": ["a.txt", "b.txt", "c.txt"]});
        let result = render_invoke(&invoke, &params, &make_special_vars()).unwrap();
        assert_eq!(result, vec!["run", "a.txt", "b.txt", "c.txt"]);
    }

    #[test]
    fn missing_param_error() {
        let invoke = make_invoke(vec!["{{nonexistent}}"]);
        let params = serde_json::json!({});
        let err = render_invoke(&invoke, &params, &make_special_vars()).unwrap_err();
        assert!(matches!(err, InvokeError::MissingParam(name) if name == "nonexistent"));
    }

    #[test]
    fn special_vars_replacement() {
        let invoke = make_invoke(vec!["{{临时目录}}/tool", "{{包目录}}/data"]);
        let params = serde_json::json!({});
        let result = render_invoke(&invoke, &params, &make_special_vars()).unwrap();
        assert_eq!(result, vec!["/tmp/fap/tool", "/opt/fap/com.example.demo/data"]);
    }

    #[test]
    fn host_data_dir_replacement() {
        let invoke = make_invoke(vec!["{{宿主数据目录}}/file"]);
        let params = serde_json::json!({});
        let result = render_invoke(&invoke, &params, &make_special_vars()).unwrap();
        assert_eq!(result, vec!["/data/host/file"]);
    }

    #[test]
    fn host_data_dir_not_configured() {
        let invoke = make_invoke(vec!["{{宿主数据目录}}/file"]);
        let params = serde_json::json!({});
        let mut vars = make_special_vars();
        vars.host_data_dir = None;
        let err = render_invoke(&invoke, &params, &vars).unwrap_err();
        assert!(matches!(err, InvokeError::HostDataDirNotConfigured));
    }

    #[test]
    fn mixed_template_and_literal() {
        let invoke = make_invoke(vec!["--input", "{{包目录}}/input.txt"]);
        let params = serde_json::json!({});
        let result = render_invoke(&invoke, &params, &make_special_vars()).unwrap();
        assert_eq!(result, vec!["--input", "/opt/fap/com.example.demo/input.txt"]);
    }
}
