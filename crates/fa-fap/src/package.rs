use crate::error::FapError;
use crate::manifest::Manifest;
use std::io::Read;
use std::path::Path;

pub struct PackageInfo {
    pub package: String,
    pub name: String,
    pub version: String,
}

pub async fn install_package(fap_path: &Path, install_dir: &Path) -> Result<Manifest, FapError> {
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

    Ok(manifest)
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
