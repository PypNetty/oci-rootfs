// src/registry/manifest.rs

// src/registry/manifest.rs

use serde::Deserialize;

/// OCI Image Index (manifest list) — ce que le registry renvoie en premier
#[derive(Debug, Deserialize)]
pub struct ImageIndex {
    pub manifests: Vec<ManifestDescriptor>,
}

/// Entrée dans l'index — une plateforme spécifique
#[derive(Debug, Deserialize)]
pub struct ManifestDescriptor {
    pub digest: String,
    pub platform: Option<Platform>,
}

#[derive(Debug, Deserialize)]
pub struct Platform {
    pub os: String,
    pub architecture: String,
}

/// Manifest individuel — décrit une image pour une plateforme
#[derive(Debug, Deserialize)]
pub struct ImageManifest {
    pub layers: Vec<LayerDescriptor>,
}

/// Un layer — on a besoin du digest pour le télécharger
#[derive(Debug, Deserialize)]
pub struct LayerDescriptor {
    pub digest: String,
    pub size: u64,
}

/// Architecture cible
#[derive(Debug, Clone)]
pub enum Arch {
    Amd64,
    Arm64,
}

impl Arch {
    pub fn as_oci_str(&self) -> &'static str {
        match self {
            Arch::Amd64 => "amd64",
            Arch::Arm64 => "arm64",
        }
    }
}

