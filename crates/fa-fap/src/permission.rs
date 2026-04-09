pub const VALID_PERMISSIONS: &[&str] = &[
    "filesystem.read",
    "filesystem.write",
    "network.outbound",
    "process.spawn",
    "clipboard.read",
    "clipboard.write",
];

pub fn validate_permissions(permissions: &[String]) -> Result<(), Vec<String>> {
    let invalid: Vec<String> = permissions
        .iter()
        .filter(|p| !VALID_PERMISSIONS.contains(&p.as_str()))
        .cloned()
        .collect();

    if invalid.is_empty() {
        Ok(())
    } else {
        Err(invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_permissions() {
        let perms: Vec<String> = VALID_PERMISSIONS.iter().map(|s| s.to_string()).collect();
        assert!(validate_permissions(&perms).is_ok());
    }

    #[test]
    fn test_invalid_permission() {
        let perms: Vec<String> = vec![
            "filesystem.read".to_string(),
            "evil.permission".to_string(),
        ];
        let result = validate_permissions(&perms);
        assert!(result.is_err());
        let invalid = result.unwrap_err();
        assert_eq!(invalid, vec!["evil.permission"]);
    }

    #[test]
    fn test_empty_permissions() {
        let perms: Vec<String> = vec![];
        assert!(validate_permissions(&perms).is_ok());
    }

    #[test]
    fn test_mixed_permissions() {
        let perms: Vec<String> = vec![
            "filesystem.read".to_string(),
            "evil.permission".to_string(),
            "network.outbound".to_string(),
            "another.bad".to_string(),
        ];
        let result = validate_permissions(&perms);
        assert!(result.is_err());
        let invalid = result.unwrap_err();
        assert_eq!(invalid.len(), 2);
        assert!(invalid.contains(&"evil.permission".to_string()));
        assert!(invalid.contains(&"another.bad".to_string()));
    }
}
