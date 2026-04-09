use crate::error::FapError;
use crate::manifest::Manifest;
use crate::version::FapVersion;
use std::io::Read;
use std::path::Path;

pub struct PackageInfo {
    pub package: String,
    pub name: String,
    pub version: String,
}

pub struct VersionChange {
    pub old_version: String,
    pub new_version: String,
}

pub struct InstallResult {
    pub manifest: Manifest,
    pub signature_verified: bool,
    pub signature_warning: Option<String>,
    pub version_change: Option<VersionChange>,
}

pub async fn install_package(
    fap_path: &Path,
    install_dir: &Path,
    verify_signature: bool,
) -> Result<InstallResult, FapError> {
    let file = std::fs::File::open(fap_path).map_err(|e| {
        FapError::InvalidFapFile(format!("failed to open fap file: {e}"))
    })?;

    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        FapError::InvalidFapFile(format!("failed to read fap as zip: {e}"))
    })?;

    let manifest_str = {
        let mut manifest_file = archive
            .by_name("manifest.json")
            .map_err(|e| FapError::InvalidFapFile(format!("manifest.json not found in archive: {e}")))?;
        let mut contents = String::new();
        manifest_file.read_to_string(&mut contents)?;
        contents
    };

    let manifest: Manifest = serde_json::from_str(&manifest_str)?;

    manifest.validate().map_err(|errors| {
        FapError::Manifest(format!("manifest validation failed: {}", errors.join(", ")))
    })?;

    let package_dir = install_dir.join(&manifest.package);

    let version_change = if package_dir.exists() {
        let old_manifest_path = package_dir.join("manifest.json");
        if old_manifest_path.exists() {
            let old_contents = std::fs::read_to_string(&old_manifest_path)?;
            let old_manifest: Manifest = serde_json::from_str(&old_contents)?;
            let old_ver = FapVersion::parse(&old_manifest.version);
            let new_ver = FapVersion::parse(&manifest.version);
            if let (Ok(old), Ok(new)) = (old_ver, new_ver) {
                if old != new {
                    Some(VersionChange {
                        old_version: old_manifest.version,
                        new_version: manifest.version.clone(),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    std::fs::create_dir_all(&package_dir)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| {
            FapError::Install(format!("failed to read archive entry: {e}"))
        })?;

        let outpath = match file.enclosed_name() {
            Some(path) => package_dir.join(path),
            None => continue,
        };

        if file.is_dir() {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    let (signature_verified, signature_warning) = if verify_signature {
        let sig_path = package_dir.join("signature.sig");
        if sig_path.exists() {
            match crate::sign::verify_package(&package_dir) {
                Ok(true) => (true, None),
                Ok(false) => {
                    std::fs::remove_dir_all(&package_dir)?;
                    return Err(FapError::Install("signature verification failed".to_string()));
                }
                Err(_) => {
                    std::fs::remove_dir_all(&package_dir)?;
                    return Err(FapError::Install("signature verification failed".to_string()));
                }
            }
        } else {
            (false, Some("包未签名，无法验证完整性".to_string()))
        }
    } else {
        (false, None)
    };

    Ok(InstallResult {
        manifest,
        signature_verified,
        signature_warning,
        version_change,
    })
}

pub async fn uninstall_package(package_name: &str, install_dir: &Path) -> Result<(), FapError> {
    let package_dir = install_dir.join(package_name);
    if !package_dir.exists() {
        return Err(FapError::PackageNotFound(package_name.to_string()));
    }
    std::fs::remove_dir_all(&package_dir)?;
    Ok(())
}

pub async fn list_packages(install_dir: &Path) -> Result<Vec<PackageInfo>, FapError> {
    let mut packages = Vec::new();

    if !install_dir.exists() {
        return Ok(packages);
    }

    for entry in std::fs::read_dir(install_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let manifest_path = path.join("manifest.json");
            if manifest_path.exists() {
                let contents = std::fs::read_to_string(&manifest_path)?;
                let manifest: Manifest = serde_json::from_str(&contents)?;
                packages.push(PackageInfo {
                    package: manifest.package,
                    name: manifest.name,
                    version: manifest.version,
                });
            }
        }
    }

    Ok(packages)
}

pub async fn inspect_package(package_name: &str, install_dir: &Path) -> Result<Manifest, FapError> {
    let manifest_path = install_dir.join(package_name).join("manifest.json");
    if !manifest_path.exists() {
        return Err(FapError::PackageNotFound(package_name.to_string()));
    }
    let contents = std::fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&contents)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_result_fields() {
        let manifest = Manifest {
            format_version: 1,
            package: "com.test.pkg".to_string(),
            name: "Test".to_string(),
            version: "1.0.0".to_string(),
            mode: crate::manifest::PackageMode::Manifest,
            platforms: vec!["windows-x86_64".to_string()],
            entry: {
                let mut map = std::collections::HashMap::new();
                map.insert("windows-x86_64".to_string(), "bin/test.exe".to_string());
                map
            },
            capabilities: {
                let mut map = std::collections::HashMap::new();
                map.insert("core".to_string(), vec![]);
                map
            },
            permissions: vec![],
            signature: None,
            lifecycle: None,
        };

        let result = InstallResult {
            signature_verified: false,
            signature_warning: Some("包未签名，无法验证完整性".to_string()),
            version_change: Some(VersionChange {
                old_version: "0.9.0".to_string(),
                new_version: "1.0.0".to_string(),
            }),
            manifest,
        };

        assert!(!result.signature_verified);
        assert!(result.signature_warning.is_some());
        assert_eq!(
            result.signature_warning.as_deref(),
            Some("包未签名，无法验证完整性")
        );
        let vc = result.version_change.unwrap();
        assert_eq!(vc.old_version, "0.9.0");
        assert_eq!(vc.new_version, "1.0.0");
        assert_eq!(result.manifest.package, "com.test.pkg");
        assert_eq!(result.manifest.version, "1.0.0");
    }
}
