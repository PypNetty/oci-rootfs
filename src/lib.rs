// src/lib.rs

pub mod image;
pub mod overlay;
pub mod registry;
pub mod store;

use std::path::PathBuf;
use std::time::Instant;
use thiserror::Error;

use image::unpack::{UnpackError, extract_layer, save_blob, save_blob_meta, verify_digest};
use overlay::fuse::{OverlayError, OverlayMount, mount_overlay, spawn_virtiofsd};
use registry::{
    auth::Credentials,
    client::{ClientError, RegistryClient},
    manifest::Arch,
};
use store::{Store, StoreError};

const MAX_ROOTFS_SIZE: u64 = 300 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum OciError {
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("client error: {0}")]
    Client(#[from] ClientError),
    #[error("unpack error: {0}")]
    Unpack(#[from] UnpackError),
    #[error("overlay error: {0}")]
    Overlay(#[from] OverlayError),
    #[error("invalid image reference: {0}")]
    InvalidRef(String),
    #[error("rootfs too large: {0} bytes, max 300MB")]
    RootfsTooLarge(u64),
}

pub struct Rootfs {
    pub merged: PathBuf,
    pub socket: PathBuf,
    _overlay: OverlayMount,
}

pub struct RootfsBuilder {
    image: String,
    arch: Arch,
    name: String,
    credentials: Option<Credentials>,
    ttl_h: u64,
}

impl Default for RootfsBuilder {
    fn default() -> Self {
        Self {
            image: String::new(),
            arch: Arch::Amd64,
            name: String::new(),
            credentials: None,
            ttl_h: 24,
        }
    }
}

impl RootfsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn image(mut self, image: &str) -> Self {
        self.image = image.to_string();
        self
    }

    pub fn arch(mut self, arch: Arch) -> Self {
        self.arch = arch;
        self
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .take(64)
            .collect();
        self
    }

    pub fn credentials(mut self, creds: Credentials) -> Self {
        self.credentials = Some(creds);
        self
    }

    pub fn ttl_hours(mut self, hours: u64) -> Self {
        self.ttl_h = hours;
        self
    }

    pub async fn build(self) -> Result<Rootfs, OciError> {
        let (registry, repository, tag) = parse_image_ref(&self.image)?;

        let store = Store::new()?;
        let vm_dir = store.vm_dir(&self.name);
        vm_dir.create()?;

        let client = RegistryClient::new(self.credentials);
        let manifest = client
            .pull_manifest(&registry, &repository, &tag, &self.arch)
            .await?;

        let bytes_total: u64 = manifest.layers.iter().map(|l| l.size).sum();
        if bytes_total > MAX_ROOTFS_SIZE {
            return Err(OciError::RootfsTooLarge(bytes_total));
        }

        let pull_start = Instant::now();
        let mut lower_dirs: Vec<PathBuf> = Vec::new();
        let mut bytes_downloaded: u64 = 0;
        let mut layers_downloaded: u32 = 0;
        let mut layers_cached: u32 = 0;

        for layer in &manifest.layers {
            let digest = layer.digest.trim_start_matches("sha256:");
            let blob_path = store.blob_path(digest);
            let layer_dir = store.blob_path(&format!("{}-extracted", digest));
            let meta_path = store.blob_path(&format!("{}-meta.json", digest));

            let needs_download = !blob_path.exists();

            if !layer_dir.exists() || !meta_path.exists() {
                let layer_start = Instant::now();

                let data = if needs_download {
                    tracing::debug!("downloading layer {}", &digest[..12]);
                    let data = client
                        .pull_blob(&registry, &repository, &layer.digest)
                        .await?;
                    bytes_downloaded += data.len() as u64;
                    layers_downloaded += 1;
                    verify_digest(&data, &layer.digest)?;
                    save_blob(&data, &blob_path)?;
                    data
                } else {
                    tracing::debug!("loading cached blob for extraction");
                    bytes::Bytes::from(std::fs::read(&blob_path).map_err(UnpackError::Io)?)
                };

                extract_layer(&data, &layer_dir)?;

                let layer_duration_ms = layer_start.elapsed().as_millis() as u64;
                save_blob_meta(
                    digest,
                    data.len() as u64,
                    layer_duration_ms,
                    self.ttl_h,
                    &meta_path,
                )?;
            } else {
                tracing::info!("layer {} cached", &digest[..12]);
                layers_cached += 1;
            }

            lower_dirs.push(layer_dir);
        }

        let pull_duration_ms = pull_start.elapsed().as_millis() as u64;

        let meta = serde_json::json!({
            "image": self.image,
            "pulled_at": chrono::Utc::now().to_rfc3339(),
            "pull_duration_ms": pull_duration_ms,
            "layers": lower_dirs.iter()
                .map(|p| p.file_name().unwrap()
                    .to_string_lossy()
                    .trim_end_matches("-extracted")
                    .to_string())
                .collect::<Vec<_>>(),
            "layers_downloaded": layers_downloaded,
            "layers_cached": layers_cached,
            "bytes_downloaded": bytes_downloaded,
            "bytes_total": bytes_total,
        });
        std::fs::write(vm_dir.base.join("manifest.json"), meta.to_string())
            .map_err(StoreError::Io)?;

        let lower_refs: Vec<&std::path::Path> = lower_dirs.iter().map(|p| p.as_path()).collect();

        let overlay = mount_overlay(
            &lower_refs,
            &vm_dir.upper(),
            &vm_dir.work(),
            &vm_dir.merged(),
        )?;

        let socket = vm_dir.merged().with_extension("sock");
        let _vhost = spawn_virtiofsd(&socket, &vm_dir.merged())?;

        Ok(Rootfs {
            merged: vm_dir.merged(),
            socket,
            _overlay: overlay,
        })
    }
}

fn parse_image_ref(image: &str) -> Result<(String, String, String), OciError> {
    let (registry, rest) = if image.contains('/') {
        let first = image.split('/').next().unwrap();
        if first.contains('.') || first.contains(':') {
            let rest = &image[first.len() + 1..];
            (first.to_string(), rest.to_string())
        } else {
            ("registry-1.docker.io".to_string(), image.to_string())
        }
    } else {
        (
            "registry-1.docker.io".to_string(),
            format!("library/{}", image),
        )
    };

    let (repository, tag) = if let Some((r, t)) = rest.split_once(':') {
        (r.to_string(), t.to_string())
    } else {
        (rest.to_string(), "latest".to_string())
    };

    Ok((registry, repository, tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_image_ref() {
        let (registry, repository, tag) = parse_image_ref("debian:trixie-slim").unwrap();
        assert_eq!(registry, "registry-1.docker.io");
        assert_eq!(repository, "library/debian");
        assert_eq!(tag, "trixie-slim");
    }

    #[tokio::test]
    #[ignore]
    async fn test_pull_debian() {
        let rootfs = RootfsBuilder::new()
            .image("debian:trixie-slim")
            .name("test-debian")
            .build()
            .await
            .unwrap();

        println!("merged: {:?}", rootfs.merged);
        println!("exists: {}", rootfs.merged.exists());
        println!("socket: {:?}", rootfs.socket);

        let os_release = rootfs.merged.join("etc/os-release");
        println!("os-release exists: {}", os_release.exists());
    }

    #[tokio::test]
    #[ignore]
    async fn test_pull_multilayer() {
        let rootfs = RootfsBuilder::new()
            .image("node:22-slim")
            .name("test-node")
            .build()
            .await
            .unwrap();

        let node_bin = rootfs.merged.join("usr/local/bin/node");
        println!("node exists: {}", node_bin.exists());
    }
}

#[tokio::test]
#[ignore]
async fn test_pull_registries() {
    let images_env = std::env::var("KYERNAL_TEST_IMAGES").unwrap_or_default();

    let images: Vec<&str> = images_env.split(',').filter(|s| !s.is_empty()).collect();

    if images.is_empty() {
        println!("set KYERNAL_TEST_IMAGES=image1:tag,image2:tag to test");
        return;
    }

    for image in images {
        let name = image.replace(['/', ':', '.'], "-");
        println!("\n--- testing {} ---", image);
        let result = RootfsBuilder::new().image(image).name(&name).build().await;

        match result {
            Ok(rootfs) => println!("✓ {} → {}", image, rootfs.merged.exists()),
            Err(e) => println!("✗ {} → {}", image, e),
        }
    }
}
