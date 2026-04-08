#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("json parse error: {0}")]
    JsonError(String),
    #[error("regex pattern required for regex parser")]
    RegexPatternRequired,
    #[error("regex error: {0}")]
    RegexError(String),
    #[error("unknown parser: {0}")]
    UnknownParser(String),
}

pub fn parse_output(
    parser: &str,
    input: &str,
    pattern: Option<&str>,
) -> Result<serde_json::Value, ParserError> {
    match parser {
        "raw" => parse_raw(input),
        "json" => parse_json(input),
        "last_line" => parse_last_line(input),
        "lines" => parse_lines(input),
        "csv" => parse_csv(input),
        "ffmpeg_progress" => parse_ffmpeg_progress(input),
        "regex" => parse_regex(input, pattern),
        _ => Err(ParserError::UnknownParser(parser.to_string())),
    }
}

fn parse_raw(input: &str) -> Result<serde_json::Value, ParserError> {
    Ok(serde_json::json!({"output": input}))
}

fn parse_json(input: &str) -> Result<serde_json::Value, ParserError> {
    serde_json::from_str(input)
        .map_err(|e| ParserError::JsonError(e.to_string()))
}

fn parse_last_line(input: &str) -> Result<serde_json::Value, ParserError> {
    let last_line = input
        .lines()
        .last()
        .unwrap_or("")
        .to_string();
    Ok(serde_json::json!({"output": last_line}))
}

fn parse_lines(input: &str) -> Result<serde_json::Value, ParserError> {
    let lines: Vec<&str> = input.lines().collect();
    Ok(serde_json::json!({"lines": lines}))
}

fn parse_csv(input: &str) -> Result<serde_json::Value, ParserError> {
    let rows: Vec<Vec<&str>> = input.lines().map(|line| line.split(',').collect()).collect();
    Ok(serde_json::json!({"rows": rows}))
}

fn parse_ffmpeg_progress(input: &str) -> Result<serde_json::Value, ParserError> {
    let mut time = String::new();

    for line in input.lines() {
        if let Some(pos) = line.find("time=") {
            let start = pos + 5;
            let rest = &line[start..];
            let end = rest
                .find(|c: char| c.is_whitespace())
                .unwrap_or(rest.len());
            time = rest[..end].to_string();
            break;
        }
    }

    let progress = if time.is_empty() {
        "unknown".to_string()
    } else {
        time.clone()
    };

    Ok(serde_json::json!({"time": time, "进度": progress}))
}

fn parse_regex(input: &str, pattern: Option<&str>) -> Result<serde_json::Value, ParserError> {
    let pat = pattern.ok_or(ParserError::RegexPatternRequired)?;

    let re = regex::Regex::new(pat)
        .map_err(|e: regex::Error| ParserError::RegexError(e.to_string()))?;

    if let Some(caps) = re.captures(input) {
        let mut groups = serde_json::Map::new();
        for (i, opt_match) in caps.iter().enumerate() {
            if let Some(m) = opt_match {
                if i == 0 {
                    groups.insert(
                        "full_match".to_string(),
                        serde_json::Value::String(m.as_str().to_string()),
                    );
                } else {
                    groups.insert(
                        format!("group_{i}"),
                        serde_json::Value::String(m.as_str().to_string()),
                    );
                }
            }
        }
        Ok(serde_json::Value::Object(groups))
    } else {
        Ok(serde_json::json!({}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_parser() {
        let result = parse_output("raw", "hello world", None).unwrap();
        assert_eq!(result["output"], "hello world");
    }

    #[test]
    fn test_json_parser() {
        let input = r#"{"key": "value", "num": 42}"#;
        let result = parse_output("json", input, None).unwrap();
        assert_eq!(result["key"], "value");
        assert_eq!(result["num"], 42);
    }

    #[test]
    fn test_json_parser_error() {
        let result = parse_output("json", "not json", None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ParserError::JsonError(_)));
    }

    #[test]
    fn test_last_line_parser() {
        let input = "line1\nline2\nline3";
        let result = parse_output("last_line", input, None).unwrap();
        assert_eq!(result["output"], "line3");
    }

    #[test]
    fn test_last_line_empty() {
        let result = parse_output("last_line", "", None).unwrap();
        assert_eq!(result["output"], "");
    }

    #[test]
    fn test_lines_parser() {
        let input = "a\nb\nc";
        let result = parse_output("lines", input, None).unwrap();
        assert_eq!(result["lines"], serde_json::json!(["a", "b", "c"]));
    }

    #[test]
    fn test_csv_parser() {
        let input = "a,b,c\n1,2,3";
        let result = parse_output("csv", input, None).unwrap();
        assert_eq!(result["rows"], serde_json::json!([["a", "b", "c"], ["1", "2", "3"]]));
    }

    #[test]
    fn test_ffmpeg_progress_parser() {
        let input = "frame=  100 fps=30 q=28.0 size=   1024kB time=00:01:23.45 bitrate= 100.0kbits/s";
        let result = parse_output("ffmpeg_progress", input, None).unwrap();
        assert_eq!(result["time"], "00:01:23.45");
    }

    #[test]
    fn test_unknown_parser() {
        let result = parse_output("unknown_parser", "input", None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ParserError::UnknownParser(_)));
    }
}
