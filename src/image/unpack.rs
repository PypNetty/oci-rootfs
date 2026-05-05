// src/image/unpack.rs

use chrono;
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
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
    if dest.symlink_metadata().map_or(false, |m| m.file_type().is_symlink()) {
        return Err(UnpackError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination is a symlink",
        )));
    }
    std::fs::create_dir_all(dest)?;

    let gz = GzDecoder::new(data);
    let mut archive = tar::Archive::new(gz);

    archive.set_preserve_permissions(true);
    archive.set_preserve_mtime(true);
    archive.set_overwrite(true);
    archive.set_unpack_xattrs(false);
    archive.set_preserve_ownerships(false);

    tracing::debug!("extracting layer to {:?}", dest);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();

        if path.is_absolute() {
            continue;
        }

        let mut skip = false;
        for component in path.components() {
            if let std::path::Component::ParentDir = component {
                skip = true;
                break;
            }
        }
        if skip {
            continue;
        }

        let target = dest.join(&path);
        if !target.starts_with(dest) {
            tracing::warn!("path traversal detected, skipping: {:?}", path);
            continue;
        }

        if let tar::EntryType::Symlink | tar::EntryType::Link = entry.header().entry_type() {
            if let Ok(Some(link_path)) = entry.link_name() {
                let link_str = link_path.to_string_lossy();
                if link_str.contains("..") {
                    tracing::warn!("unsafe symlink skipped: {:?} -> {:?}", path, link_str);
                    continue;
                }
            }
        }

        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }

        if let Some(parent) = target.parent() {
            if !parent.exists() {
                tracing::debug!("creating parent dir: {:?}", parent);
                std::fs::create_dir_all(parent)?;
            }
        }

        if entry.header().entry_type().is_hard_link() {
            if let Err(e) = entry.unpack(&target) {
                tracing::debug!("hard link skipped: {:?} — {}", path, e);
            }
            continue;
        }

        if target.exists() {
            if target.is_dir() {
                let _ = std::fs::remove_dir_all(&target);
            } else {
                let _ = std::fs::remove_file(&target);
            }
        }

        entry.unpack(&target)?;

        if let Ok(metadata) = fs::metadata(&target) {
            let mode = metadata.mode();
            if mode & 0o4000 != 0 || mode & 0o2000 != 0 {
                let _ = fs::set_permissions(&target, fs::Permissions::from_mode(mode & !0o7111));
            }
        }
    }

    Ok(())
}

pub fn save_blob(data: &[u8], dest: &Path) -> Result<(), UnpackError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if dest.symlink_metadata().map_or(false, |m| m.file_type().is_symlink()) {
        return Err(UnpackError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination is a symlink",
        )));
    }
    if dest.exists() {
        let existing = std::fs::read(dest)?;
        if existing == data {
            return Ok(());
        }
        std::fs::write(dest, data)?;
    } else {
        let file = fs::File::options()
            .create_new(true)
            .write(true)
            .open(dest)?;
        let mut writer = std::io::BufWriter::new(file);
        writer.write_all(data)?;
        writer.flush()?;
    }
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
    let tmp = dest.with_extension("tmp");
    std::fs::write(&tmp, meta.to_string())?;
    std::fs::rename(tmp, dest)?;
    Ok(())
}
