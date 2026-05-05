// src/overlay/fuse.rs

use std::path::Path;
use std::process::{Child, Command};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OverlayError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("fuse-overlayfs not found — install fuse-overlayfs")]
    NotFound,
    #[error("virtiofsd not found — install virtiofsd")]
    VirtiofsdNotFound,
    #[error("fuse-overlayfs exited with status {0}")]
    MountFailed(i32),
    #[error("invalid path: {0}")]
    InvalidPath(String),
}

fn safe_path_str(p: &Path) -> Result<&str, OverlayError> {
    let s = p
        .to_str()
        .ok_or_else(|| OverlayError::InvalidPath(p.display().to_string()))?;
    if s.contains(':') || s.contains(',') {
        return Err(OverlayError::InvalidPath(p.display().to_string()));
    }
    Ok(s)
}

pub struct OverlayMount {
    merged: std::path::PathBuf,
}

impl Drop for OverlayMount {
    fn drop(&mut self) {
        let merged = self.merged.to_string_lossy();
        let _ = Command::new("fusermount3")
            .args(["-u", merged.as_ref()])
            .status();
    }
}

fn find_virtiofsd() -> Option<&'static str> {
    let candidates = [
        "/usr/libexec/virtiofsd",
        "/usr/lib/qemu/virtiofsd",
        "/usr/local/bin/virtiofsd",
    ];
    candidates.iter().find(|p| Path::new(p).exists()).copied()
}

pub fn mount_overlay(
    lower_dirs: &[&Path],
    upper: &Path,
    work: &Path,
    merged: &Path,
) -> Result<OverlayMount, OverlayError> {
    which("fuse-overlayfs").ok_or(OverlayError::NotFound)?;

    let lowers: Vec<&str> = lower_dirs
        .iter()
        .map(|p| safe_path_str(p))
        .collect::<Result<Vec<_>, _>>()?;

    let lowers_str = lowers.join(":");
    let upper_str = safe_path_str(upper)?;
    let work_str = safe_path_str(work)?;
    let merged_str = safe_path_str(merged)?;

    let status = Command::new("fuse-overlayfs")
        .arg("-o")
        .arg(format!(
            "lowerdir={},upperdir={},workdir={}",
            lowers_str, upper_str, work_str
        ))
        .arg(merged_str)
        .status()?;

    if !status.success() {
        return Err(OverlayError::MountFailed(status.code().unwrap_or(-1)));
    }

    Ok(OverlayMount {
        merged: merged.to_path_buf(),
    })
}

pub fn spawn_virtiofsd(socket: &Path, shared_dir: &Path) -> Result<Child, OverlayError> {
    let bin = find_virtiofsd().ok_or(OverlayError::VirtiofsdNotFound)?;

    let socket_str = safe_path_str(socket)?;
    let shared_dir_str = safe_path_str(shared_dir)?;

    let child = Command::new(bin)
        .args([
            "--socket-path",
            socket_str,
            "--shared-dir",
            shared_dir_str,
            "--cache",
            "auto",
            "--sandbox",
            "namespace",
            "--thread-pool-size",
            "1",
            "--flock",
            "--no-allow-foreign-sync",
        ])
        .spawn()?;

    Ok(child)
}

fn which(bin: &str) -> Option<()> {
    Command::new("which")
        .arg(bin)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| ())
}
