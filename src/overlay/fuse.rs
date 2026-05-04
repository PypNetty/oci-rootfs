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
}

pub struct OverlayMount {
    merged: std::path::PathBuf,
}

impl Drop for OverlayMount {
    fn drop(&mut self) {
        let _ = Command::new("fusermount3")
            .args(["-u", self.merged.to_str().unwrap()])
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

    let lowers = lower_dirs
        .iter()
        .map(|p| p.to_str().unwrap())
        .collect::<Vec<_>>()
        .join(":");

    let status = Command::new("fuse-overlayfs")
        .arg("-o")
        .arg(format!(
            "lowerdir={},upperdir={},workdir={}",
            lowers,
            upper.display(),
            work.display()
        ))
        .arg(merged.to_str().unwrap())
        .status()?;

    if !status.success() {
        return Err(OverlayError::MountFailed(
            status.code().unwrap_or(-1),
        ));
    }

    Ok(OverlayMount {
        merged: merged.to_path_buf(),
    })
}

pub fn spawn_virtiofsd(
    socket: &Path,
    shared_dir: &Path,
) -> Result<Child, OverlayError> {
    let bin = find_virtiofsd().ok_or(OverlayError::VirtiofsdNotFound)?;

    let child = Command::new(bin)
        .args([
            "--socket-path",
            socket.to_str().unwrap(),
            "--shared-dir",
            shared_dir.to_str().unwrap(),
            "--cache",
            "auto",
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