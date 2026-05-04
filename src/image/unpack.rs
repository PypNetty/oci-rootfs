// src/image/unpack.rs

use chrono;
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UnpackError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("digest mismatch: expected {expected}, got {got}")]
    DigestMismatch { expected: String, got: String },
}

pub fn verify_digest(data: &[u8], expected: &str) -> Result<(), UnpackError> {
    let expected = expected.trim_start_matches("sha256:");
    let got = hex::encode(Sha256::digest(data));
    if got != expected {
        return Err(UnpackError::DigestMismatch {
            expected: expected.to_string(),
            got,
        });
    }
    Ok(())
}

pub fn extract_layer(data: &[u8], dest: &Path) -> Result<(), UnpackError> {
    std::fs::create_dir_all(dest)?;

    let gz = GzDecoder::new(data);
    let mut archive = tar::Archive::new(gz);

    archive.set_preserve_permissions(true);
    archive.set_preserve_mtime(true);
    archive.set_overwrite(true);
    archive.set_unpack_xattrs(false);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        if path.is_absolute() {
            continue;
        }

        for component in path.components() {
            match component {
                std::path::Component::ParentDir => continue,
                std::path::Component::Normal(_) => {}
                _ => continue,
            }
        }

        let target = dest.join(&path);
        if !target.starts_with(dest) {
            tracing::warn!("path traversal detected, skipping: {:?}", path);
            continue;
        }

        entry.unpack(&target)?;
    }

    Ok(())
}

pub fn save_blob(data: &[u8], dest: &Path) -> Result<(), UnpackError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, data)?;
    Ok(())
}

pub fn save_blob_meta(
    digest: &str,
    size: u64,
    duration_ms: u64,
    ttl_hours: u64,
    dest: &Path,
) -> Result<(), UnpackError> {
    let now = chrono::Utc::now();
    let expires_at = now + chrono::Duration::hours(ttl_hours as i64);

    let meta = serde_json::json!({
        "digest": digest,
        "size": size,
        "duration_ms": duration_ms,
        "pulled_at": now.to_rfc3339(),
        "expires_at": expires_at.to_rfc3339(),
        "ttl_hours": ttl_hours,
    });

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, meta.to_string())?;
    Ok(())
}