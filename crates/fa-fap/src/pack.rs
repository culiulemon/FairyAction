use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::manifest::Manifest;

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip error: {0}")]
    Zip(String),
    #[error("package directory not found: {0}")]
    PackageDirNotFound(String),
}

pub fn pack_fap(
    package_dir: &Path,
    output_path: Option<&Path>,
    verify_signature: bool,
) -> Result<PathBuf, PackError> {
    if !package_dir.is_dir() {
        return Err(PackError::PackageDirNotFound(
            package_dir.display().to_string(),
        ));
    }

    let manifest_path = package_dir.join("manifest.json");
    let manifest_str = std::fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&manifest_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let package_name = manifest.name;

    if verify_signature {
        let sig_path = package_dir.join("signature.sig");
        if sig_path.exists() {
            match crate::sign::verify_package(package_dir) {
                Ok(true) => {}
                Ok(false) => {
                    tracing::warn!("no valid signature found, continuing to pack");
                }
                Err(e) => {
                    return Err(PackError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("signature verification failed: {e}"),
                    )));
                }
            }
        }
    }

    let output = resolve_output_path(output_path, &package_name);

    let file = File::create(&output)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    add_directory_to_zip(&mut writer, package_dir, package_dir, &options)?;

    writer.finish().map_err(|e| PackError::Zip(e.to_string()))?;

    Ok(output)
}

fn resolve_output_path(output_path: Option<&Path>, package_name: &str) -> PathBuf {
    match output_path {
        Some(path) if path.is_dir() || path.extension().is_none() => {
            path.join(format!("{package_name}.fap"))
        }
        Some(path) => path.to_path_buf(),
        None => PathBuf::from(format!("{package_name}.fap")),
    }
}

fn add_directory_to_zip<W: std::io::Write + std::io::Seek>(
    writer: &mut zip::ZipWriter<W>,
    base_dir: &Path,
    current_dir: &Path,
    options: &zip::write::SimpleFileOptions,
) -> Result<(), PackError> {
    let entries = std::fs::read_dir(current_dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if file_name.starts_with('.') {
            continue;
        }

        let relative = path.strip_prefix(base_dir).unwrap_or(&path);

        if path.is_dir() {
            add_directory_to_zip(writer, base_dir, &path, options)?;
        } else {
            let mut file = File::open(&path)?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;

            let zip_path = relative.to_string_lossy().replace('\\', "/");
            writer
                .start_file(zip_path, *options)
                .map_err(|e| PackError::Zip(e.to_string()))?;
            writer.write_all(&buf)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_package_dir(dir: &Path) {
        fs::create_dir_all(dir).unwrap();

        let manifest = r#"{
    "format_version": 1,
    "package": "com.test.pack",
    "name": "Test Pack",
    "version": "1.0.0",
    "mode": "manifest",
    "platforms": ["windows-x86_64"],
    "entry": {"windows-x86_64": "bin/test.exe"},
    "capabilities": {"core": [{"名称": "test", "动作": []}]}
}"#;
        fs::write(dir.join("manifest.json"), manifest).unwrap();

        let bin_dir = dir.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::write(bin_dir.join("test.exe"), b"fake binary").unwrap();

        let resources_dir = dir.join("resources");
        fs::create_dir_all(&resources_dir).unwrap();
        fs::write(resources_dir.join("icon.png"), b"fake icon").unwrap();
    }

    #[test]
    fn test_pack_fap_creates_zip() {
        let tmp = tempfile::tempdir().unwrap();
        let package_dir = tmp.path().join("test_package");
        create_test_package_dir(&package_dir);

        let output_dir = tmp.path().join("output");
        fs::create_dir_all(&output_dir).unwrap();

        let result = pack_fap(&package_dir, Some(&output_dir), false);

        assert!(result.is_ok(), "pack_fap failed: {:?}", result.err());
        let fap_path = result.unwrap();
        assert!(fap_path.exists(), "fap file does not exist");
        assert_eq!(
            fap_path.file_name().unwrap(),
            "Test Pack.fap"
        );

        let file = File::open(&fap_path).unwrap();
        let archive = zip::ZipArchive::new(file);
        assert!(archive.is_ok(), "not a valid zip: {:?}", archive.err());

        let archive = archive.unwrap();
        let names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
        assert!(names.iter().any(|n| n == "manifest.json"));
        assert!(names.iter().any(|n| n == "bin/test.exe"));
        assert!(names.iter().any(|n| n == "resources/icon.png"));
    }

    #[test]
    fn test_pack_fap_excludes_hidden_files() {
        let tmp = tempfile::tempdir().unwrap();
        let package_dir = tmp.path().join("test_package");
        create_test_package_dir(&package_dir);

        fs::write(package_dir.join(".hidden_file"), b"hidden").unwrap();
        let hidden_dir = package_dir.join(".hidden_dir");
        fs::create_dir_all(&hidden_dir).unwrap();
        fs::write(hidden_dir.join("secret.txt"), b"secret").unwrap();

        let output_path = tmp.path().join("output.fap");
        let result = pack_fap(&package_dir, Some(&output_path), false);

        assert!(result.is_ok(), "pack_fap failed: {:?}", result.err());

        let file = File::open(output_path).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();

        assert!(!names.iter().any(|n| n == ".hidden_file"));
        assert!(!names.iter().any(|n| n == ".hidden_dir/secret.txt"));
        assert!(!names.iter().any(|n| n.contains(".hidden_dir")));
    }
}
