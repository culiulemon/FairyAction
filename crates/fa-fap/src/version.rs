use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FapVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum VersionError {
    #[error("invalid version format: {0}")]
    InvalidFormat(String),
}

impl FapVersion {
    pub fn parse(s: &str) -> Result<FapVersion, VersionError> {
        let s = s.strip_prefix('v').unwrap_or(s);
        let parts: Vec<&str> = s.split('.').collect();

        if parts.is_empty() {
            return Err(VersionError::InvalidFormat(s.to_string()));
        }

        let major = parts[0]
            .parse::<u32>()
            .map_err(|_| VersionError::InvalidFormat(s.to_string()))?;

        let minor = parts
            .get(1)
            .map(|p| p.parse::<u32>())
            .transpose()
            .map_err(|_| VersionError::InvalidFormat(s.to_string()))?
            .unwrap_or(0);

        let patch = parts
            .get(2)
            .map(|p| p.parse::<u32>())
            .transpose()
            .map_err(|_| VersionError::InvalidFormat(s.to_string()))?
            .unwrap_or(0);

        Ok(FapVersion {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for FapVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Ord for FapVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
    }
}

impl PartialOrd for FapVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_version() {
        let v = FapVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_parse_version_with_v_prefix() {
        let v = FapVersion::parse("v2.0.1").unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 1);
    }

    #[test]
    fn test_parse_two_part_version() {
        let v = FapVersion::parse("1.5").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 5);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_parse_one_part_version() {
        let v = FapVersion::parse("3").unwrap();
        assert_eq!(v.major, 3);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_parse_invalid_version() {
        assert!(FapVersion::parse("abc").is_err());
    }

    #[test]
    fn test_version_ordering() {
        let v1 = FapVersion::parse("1.2.3").unwrap();
        let v2 = FapVersion::parse("1.2.4").unwrap();
        let v3 = FapVersion::parse("1.3.0").unwrap();
        let v4 = FapVersion::parse("2.0.0").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);
    }

    #[test]
    fn test_version_equality() {
        let v1 = FapVersion::parse("1.2.3").unwrap();
        let v2 = FapVersion::parse("1.2.3").unwrap();
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_version_display() {
        let v = FapVersion::parse("1.2.3").unwrap();
        assert_eq!(format!("{}", v), "1.2.3");
    }
}
