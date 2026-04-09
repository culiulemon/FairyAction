use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format_version: u32,
    pub package: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "mode")]
    pub mode: PackageMode,
    pub platforms: Vec<String>,
    pub entry: HashMap<String, String>,
    #[serde(default)]
    pub capabilities: HashMap<String, Vec<CapabilityDomain>>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub signature: Option<SignatureInfo>,
    #[serde(default)]
    pub lifecycle: Option<Lifecycle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageMode {
    Manifest,
    Sdk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lifecycle {
    Oneshot,
    Persistent,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDomain {
    pub 名称: String,
    pub 动作: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub 名称: String,
    #[serde(default)]
    pub 参数: HashMap<String, ParamDef>,
    #[serde(default)]
    pub invoke: Option<InvokeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    #[serde(rename = "类型")]
    pub param_type: String,
    #[serde(default)]
    pub 默认: Option<serde_json::Value>,
    #[serde(default)]
    pub 必填: Option<bool>,
    #[serde(default)]
    pub 描述: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeConfig {
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub exit_code: Option<HashMap<String, String>>,
    #[serde(default)]
    pub output: Option<OutputConfig>,
    #[serde(default)]
    pub timeout: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_source")]
    pub source: String,
    pub parser: String,
    #[serde(default)]
    pub pattern: Option<String>,
}

fn default_source() -> String {
    "stdout".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureInfo {
    pub algorithm: String,
    pub value: String,
    pub public_key: Option<String>,
}

impl Manifest {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.package.trim().is_empty() {
            errors.push("package must not be empty".to_string());
        }

        if self.platforms.is_empty() {
            errors.push("platforms must not be empty".to_string());
        }

        if self.entry.is_empty() {
            errors.push("entry must have at least one platform".to_string());
        }

        for platform in self.entry.keys() {
            if !self.platforms.contains(platform) {
                errors.push(format!(
                    "entry platform '{platform}' is not listed in platforms"
                ));
            }
        }

        if matches!(self.mode, PackageMode::Manifest) && self.capabilities.is_empty() {
            errors.push("capabilities must not be empty for manifest mode".to_string());
        }

        let perm_errors = crate::permission::validate_permissions(&self.permissions);
        if let Err(invalid) = perm_errors {
            errors.extend(invalid.into_iter().map(|p| format!("unknown permission: {p}")));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest_json() -> &'static str {
        r#"{
            "format_version": 1,
            "package": "com.example.demo",
            "name": "Demo Package",
            "version": "1.0.0",
            "mode": "manifest",
            "platforms": ["windows-x86_64", "linux-x86_64"],
            "entry": {
                "windows-x86_64": "bin/demo.exe",
                "linux-x86_64": "bin/demo"
            },
            "capabilities": {
                "core": [
                    {
                        "名称": "file",
                        "动作": [
                            {
                                "名称": "read",
                                "参数": {
                                    "path": {
                                        "类型": "string",
                                        "必填": true,
                                        "描述": "文件路径"
                                    }
                                },
                                "invoke": {
                                    "args": ["{{来源路径的父目录}}/bin/demo.exe", "read", "{{path}}"],
                                    "timeout": 30
                                }
                            }
                        ]
                    }
                ]
            },
            "permissions": ["filesystem.read"],
            "lifecycle": "oneshot"
        }"#
    }

    #[test]
    fn parse_full_manifest() {
        let manifest: Manifest = serde_json::from_str(sample_manifest_json()).unwrap();
        assert_eq!(manifest.format_version, 1);
        assert_eq!(manifest.package, "com.example.demo");
        assert_eq!(manifest.name, "Demo Package");
        assert_eq!(manifest.version, "1.0.0");
        assert!(matches!(manifest.mode, PackageMode::Manifest));
        assert_eq!(manifest.platforms.len(), 2);
        assert_eq!(manifest.entry.len(), 2);
        assert!(manifest.capabilities.contains_key("core"));
        assert_eq!(manifest.permissions.len(), 1);
        assert!(manifest.signature.is_none());
        assert!(matches!(manifest.lifecycle, Some(Lifecycle::Oneshot)));
    }

    #[test]
    fn validate_valid_manifest() {
        let manifest: Manifest = serde_json::from_str(sample_manifest_json()).unwrap();
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn validate_missing_package() {
        let json = r#"{
            "format_version": 1,
            "package": "",
            "name": "Test",
            "version": "1.0.0",
            "mode": "manifest",
            "platforms": ["windows-x86_64"],
            "entry": {"windows-x86_64": "bin/test.exe"},
            "capabilities": {"core": []}
        }"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        let errors = manifest.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("package must not be empty")));
    }

    #[test]
    fn validate_empty_platforms() {
        let json = r#"{
            "format_version": 1,
            "package": "com.test",
            "name": "Test",
            "version": "1.0.0",
            "mode": "manifest",
            "platforms": [],
            "entry": {},
            "capabilities": {"core": []}
        }"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        let errors = manifest.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("platforms must not be empty")));
    }

    #[test]
    fn validate_entry_platform_not_in_platforms() {
        let json = r#"{
            "format_version": 1,
            "package": "com.test",
            "name": "Test",
            "version": "1.0.0",
            "mode": "manifest",
            "platforms": ["linux-x86_64"],
            "entry": {"windows-x86_64": "bin/test.exe"},
            "capabilities": {"core": []}
        }"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        let errors = manifest.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.contains("entry platform 'windows-x86_64' is not listed in platforms")));
    }

    #[test]
    fn validate_manifest_mode_empty_capabilities() {
        let json = r#"{
            "format_version": 1,
            "package": "com.test",
            "name": "Test",
            "version": "1.0.0",
            "mode": "manifest",
            "platforms": ["windows-x86_64"],
            "entry": {"windows-x86_64": "bin/test.exe"},
            "capabilities": {}
        }"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        let errors = manifest.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.contains("capabilities must not be empty for manifest mode")));
    }

    #[test]
    fn validate_invalid_permission() {
        let json = r#"{
            "format_version": 1,
            "package": "com.test",
            "name": "Test",
            "version": "1.0.0",
            "mode": "manifest",
            "platforms": ["windows-x86_64"],
            "entry": {"windows-x86_64": "bin/test.exe"},
            "capabilities": {"core": []},
            "permissions": ["filesystem.read", "evil.permission"]
        }"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        let errors = manifest.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.contains("unknown permission: evil.permission")));
    }
}
