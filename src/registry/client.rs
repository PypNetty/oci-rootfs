// src/registry/client.rs

use reqwest::{Client, StatusCode};
use thiserror::Error;

const MAX_LAYER_SIZE: u64 = 100 * 1024 * 1024; // 100 Mo

use crate::registry::auth::{self, AuthError, Credentials};
use crate::registry::manifest::{Arch, ImageIndex, ImageManifest};

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("auth error: {0}")]
    Auth(#[from] AuthError),
    #[error("manifest not found for arch {0}")]
    ArchNotFound(String),
    #[error("layer too large: {0} bytes")]
    LayerTooLarge(u64),
    #[error("unexpected status {0}")]
    UnexpectedStatus(u16),
}

pub struct RegistryClient {
    client: Client,
    credentials: Option<Credentials>,
}

impl RegistryClient {
    pub fn new(credentials: Option<Credentials>) -> Self {
        Self {
            client: Client::new(),
            credentials,
        }
    }

    /// Pull le manifest list puis sélectionne le bon manifest pour l'arch cible
    pub async fn pull_manifest(
        &self,
        registry: &str,
        repository: &str,
        reference: &str,
        arch: &Arch,
    ) -> Result<ImageManifest, ClientError> {
        let token = self.get_token(registry, repository).await?;

        let url = format!(
            "https://{}/v2/{}/manifests/{}",
            registry, repository, reference
        );

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .header(
                "Accept",
                "application/vnd.oci.image.index.v1+json, \
                 application/vnd.docker.distribution.manifest.list.v2+json",
            )
            .send()
            .await?;

        let index: ImageIndex = resp.json().await?;

        // Sélectionne le digest pour l'arch cible
        let descriptor = index
            .manifests
            .iter()
            .find(|m| {
                m.platform
                    .as_ref()
                    .map(|p| p.architecture == arch.as_oci_str() && p.os == "linux")
                    .unwrap_or(false)
            })
            .ok_or_else(|| ClientError::ArchNotFound(arch.as_oci_str().to_string()))?;

        // Pull le manifest individuel
        let url = format!(
            "https://{}/v2/{}/manifests/{}",
            registry, repository, descriptor.digest
        );

        let manifest: ImageManifest = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .header(
                "Accept",
                "application/vnd.oci.image.manifest.v1+json, \
                 application/vnd.docker.distribution.manifest.v2+json",
            )
            .send()
            .await?
            .json()
            .await?;

        Ok(manifest)
    }

/// Télécharge un blob (layer) et retourne les bytes
    pub async fn pull_blob(
        &self,
        registry: &str,
        repository: &str,
        digest: &str,
    ) -> Result<bytes::Bytes, ClientError> {
        let token = self.get_token(registry, repository).await?;

        let url = format!(
            "https://{}/v2/{}/blobs/{}",
            registry, repository, digest
        );

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await?;

        // Vérifie la taille déclarée avant de télécharger
        if let Some(content_length) = resp.content_length() {
            if content_length > MAX_LAYER_SIZE {
                return Err(ClientError::LayerTooLarge(content_length));
            }
        }

        match resp.status() {
            StatusCode::OK => Ok(resp.bytes().await?),
            s => Err(ClientError::UnexpectedStatus(s.as_u16())),
        }
    }

    async fn get_token(
        &self,
        registry: &str,
        repository: &str,
    ) -> Result<String, ClientError> {
        let scope = format!("repository:{}:pull", repository);

        // Docker Hub a un realm spécifique
        let (realm, service) = if registry.contains("docker.io") {
            (
                "https://auth.docker.io/token".to_string(),
                "registry.docker.io".to_string(),
            )
        } else {
            // Pour les autres registries on fait une requête anonyme
            // et on parse le WWW-Authenticate si 401
            (
                format!("https://{}/token", registry),
                registry.to_string(),
            )
        };

        let token = auth::fetch_token(
            &self.client,
            &realm,
            &service,
            &scope,
            self.credentials.as_ref(),
        )
        .await?;

        Ok(token)
    }
}