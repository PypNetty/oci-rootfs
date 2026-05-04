// src/store/mod.rs

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]

pub enum StoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Layout :
/// ~/.local/share/kyernal/
/// ├── blobs/sha256/      ← layers OCI dédupliqués
/// └── vms/
///     └── {name}/
///         ├── upper/     ← writes de la VM
///         ├── work/      ← workdir overlayfs
///         └── merged/    ← virtiofsd pointe ici
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new() -> Result<Self, StoreError> {
        let root = dirs_next::data_dir()
            .unwrap_or_else(|| PathBuf::from("/var/lib/kyernal"))
            .join("kyernal");

        std::fs::create_dir_all(root.join("blobs/sha256"))?;
        std::fs::create_dir_all(root.join("vms"))?;

        Ok(Self { root })
    }

    pub fn blob_path(&self, digest: &str) -> PathBuf {
        self.root.join("blobs/sha256").join(digest)
    }

    pub fn vm_dir(&self, name: &str) -> VmDir {
        VmDir::new(self.root.join("vms").join(name))
    }
}

pub struct VmDir {
    pub base: PathBuf,
}

impl VmDir {
    fn new(base: PathBuf) -> Self {
        Self { base }
    }

    pub fn upper(&self) -> PathBuf {
        self.base.join("upper")
    }
    pub fn work(&self) -> PathBuf {
        self.base.join("work")
    }
    pub fn merged(&self) -> PathBuf {
        self.base.join("merged")
    }

    pub fn create(&self) -> Result<(), StoreError> {
        std::fs::create_dir_all(self.upper())?;
        std::fs::create_dir_all(self.work())?;
        std::fs::create_dir_all(self.merged())?;
        Ok(())
    }
}
