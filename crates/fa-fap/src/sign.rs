use std::fs;
use std::io::Write;
use std::path::Path;

use base64::Engine;
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::manifest::{Manifest, SignatureInfo};

#[derive(Debug, thiserror::Error)]
pub enum SignError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("key error: {0}")]
    Key(String),
    #[error("signature error: {0}")]
    Signature(String),
    #[error("manifest error: {0}")]
    Manifest(String),
}

pub struct Keypair {
    pub private_key: Vec<u8>,
    pub public_key: Vec<u8>,
}

pub fn generate_keypair() -> Result<Keypair, SignError> {
    let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
    let verifying_key = signing_key.verifying_key();

    let mut private_key = Vec::with_capacity(64);
    private_key.extend_from_slice(&signing_key.to_bytes());
    private_key.extend_from_slice(&verifying_key.to_bytes());

    Ok(Keypair {
        private_key,
        public_key: verifying_key.to_bytes().to_vec(),
    })
}

pub fn write_keypair(keypair: &Keypair, output_dir: &Path) -> Result<(), SignError> {
    fs::create_dir_all(output_dir)?;

    let private_path = output_dir.join("fap_private.key");
    let mut private_file = fs::File::create(&private_path)?;
    private_file.write_all(&keypair.private_key)?;

    let public_path = output_dir.join("fap_public.key");
    let mut public_file = fs::File::create(&public_path)?;
    public_file.write_all(&keypair.public_key)?;

    Ok(())
}

pub fn compute_digest(package_dir: &Path) -> Result<[u8; 32], SignError> {
    let mut entries = Vec::new();
    collect_files(package_dir, package_dir, &mut entries)?;
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut concat = String::new();
    for (relative_path, full_path) in &entries {
        let data = fs::read(full_path)?;
        let hash = Sha256::digest(&data);
        let hex_hash = format!("{:x}", hash);
        concat.push_str(&format!("{}:{}\n", relative_path, hex_hash));
    }

    let final_hash = Sha256::digest(concat.as_bytes());
    Ok(final_hash.into())
}

fn collect_files(base: &Path, current: &Path, entries: &mut Vec<(String, std::path::PathBuf)>) -> Result<(), SignError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(base, &path, entries)?;
        } else {
            let relative = path.strip_prefix(base).map_err(|e| {
                SignError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?;
            let relative_str = relative.to_string_lossy().to_string();
            if relative_str != "signature.sig" && relative_str != "manifest.json" {
                entries.push((relative_str, path));
            }
        }
    }
    Ok(())
}

pub fn sign_package(private_key_path: &Path, package_dir: &Path) -> Result<(), SignError> {
    let private_key_bytes = fs::read(private_key_path)?;
    if private_key_bytes.len() != 64 {
        return Err(SignError::Key(format!(
            "invalid private key length: expected 64 bytes, got {}",
            private_key_bytes.len()
        )));
    }

    let signing_key = SigningKey::from_bytes(
        private_key_bytes[..32].try_into().map_err(|_| {
            SignError::Key("failed to parse private key seed".to_string())
        })?,
    );
    let public_key_bytes = signing_key.verifying_key().to_bytes();

    let digest = compute_digest(package_dir)?;
    let signature = signing_key.sign(&digest);

    let sig_path = package_dir.join("signature.sig");
    fs::write(&sig_path, signature.to_bytes())?;

    let manifest_path = package_dir.join("manifest.json");
    let manifest_content = fs::read_to_string(&manifest_path)?;
    let mut manifest: Manifest = serde_json::from_str(&manifest_content)
        .map_err(|e| SignError::Manifest(e.to_string()))?;

    manifest.signature = Some(SignatureInfo {
        algorithm: "Ed25519".to_string(),
        value: base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()),
        public_key: Some(base64::engine::general_purpose::STANDARD.encode(public_key_bytes)),
    });

    let updated = serde_json::to_string_pretty(&manifest)
        .map_err(|e| SignError::Manifest(e.to_string()))?;
    fs::write(&manifest_path, updated)?;

    Ok(())
}

pub fn verify_package(package_dir: &Path) -> Result<bool, SignError> {
    let manifest_path = package_dir.join("manifest.json");
    let manifest_content = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&manifest_content)
        .map_err(|e| SignError::Manifest(e.to_string()))?;

    let sig_info = match &manifest.signature {
        Some(info) => info,
        None => return Ok(false),
    };

    let public_key_b64 = sig_info.public_key.as_deref().ok_or_else(|| {
        SignError::Signature("missing public_key in manifest".to_string())
    })?;
    let public_key_bytes = base64::engine::general_purpose::STANDARD
        .decode(public_key_b64)
        .map_err(|e| SignError::Signature(format!("invalid public_key base64: {e}")))?;

    let verifying_key = VerifyingKey::from_bytes(
        public_key_bytes[..32].try_into().map_err(|_| {
            SignError::Signature("invalid public key length".to_string())
        })?,
    )
    .map_err(|e| SignError::Signature(format!("invalid public key: {e}")))?;

    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(&sig_info.value)
        .map_err(|e| SignError::Signature(format!("invalid signature base64: {e}")))?;

    let signature = ed25519_dalek::Signature::from_slice(&sig_bytes)
        .map_err(|e| SignError::Signature(format!("invalid signature: {e}")))?;

    let digest = compute_digest(package_dir)?;
    match verifying_key.verify(&digest, &signature) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn test_generate_keypair() {
        let keypair = generate_keypair().unwrap();
        assert_eq!(keypair.private_key.len(), 64);
        assert_eq!(keypair.public_key.len(), 32);
    }

    #[test]
    fn test_compute_digest_empty_dir() {
        let dir = setup_temp_dir();
        let digest = compute_digest(dir.path()).unwrap();
        assert_ne!(digest, [0u8; 32]);
    }

    #[test]
    fn test_compute_digest_with_files() {
        let dir = setup_temp_dir();
        fs::write(dir.path().join("a.txt"), b"hello").unwrap();
        fs::write(dir.path().join("b.txt"), b"world").unwrap();
        let digest = compute_digest(dir.path()).unwrap();
        assert_ne!(digest, [0u8; 32]);

        let digest2 = compute_digest(dir.path()).unwrap();
        assert_eq!(digest, digest2);
    }

    #[test]
    fn test_sign_and_verify() {
        let dir = setup_temp_dir();
        let key_dir = setup_temp_dir();

        let keypair = generate_keypair().unwrap();
        write_keypair(&keypair, key_dir.path()).unwrap();

        let manifest_json = r#"{
            "format_version": 1,
            "package": "com.test.sign",
            "name": "Test Sign",
            "version": "1.0.0",
            "mode": "manifest",
            "platforms": ["windows-x86_64"],
            "entry": {"windows-x86_64": "bin/test.exe"},
            "capabilities": {"core": []}
        }"#;
        fs::write(dir.path().join("manifest.json"), manifest_json).unwrap();
        fs::write(dir.path().join("data.txt"), b"test data").unwrap();

        let private_key_path = key_dir.path().join("fap_private.key");
        sign_package(&private_key_path, dir.path()).unwrap();

        assert!(dir.path().join("signature.sig").exists());

        let verified = verify_package(dir.path()).unwrap();
        assert!(verified);
    }
}
