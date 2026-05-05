// src/registry/client.rs

use reqwest::Client;
use sha2::{Digest, Sha256};
use std::time::Duration;
use thiserror::Error;

const MAX_LAYER_SIZE: u64 = 100 * 1024 * 1024; // 100 Mo
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

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
    #[error("digest mismatch: expected {expected}, got {got}")]
    DigestMismatch { expected: String, got: String },
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct RegistryClient {
    client: Client,
    credentials: Option<Credentials>,
}

impl RegistryClient {
    pub fn new(credentials: Option<Credentials>) -> Self {
        let client = Client::builder()
            .timeout(HTTP_TIMEOUT)
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
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

        let resp = self
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
            .error_for_status()?;

        let body_bytes = resp.bytes().await?;
        let manifest: ImageManifest = serde_json::from_slice(&body_bytes)?;

        // Verify manifest digest
        let manifest_digest = hex::encode(Sha256::digest(&body_bytes));
        let expected_digest = descriptor.digest.trim_start_matches("sha256:");
        if manifest_digest != expected_digest {
            return Err(ClientError::DigestMismatch {
                expected: expected_digest.to_string(),
                got: manifest_digest,
            });
        }

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

        let url = format!("https://{}/v2/{}/blobs/{}", registry, repository, digest);

        let resp = self.client.get(&url).bearer_auth(&token).send().await?;

        let mut buf = Vec::with_capacity(MAX_LAYER_SIZE as usize);
        let mut stream = resp.bytes_stream();

        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if buf.len() + chunk.len() > MAX_LAYER_SIZE as usize {
                return Err(ClientError::LayerTooLarge((buf.len() + chunk.len()) as u64));
            }
            buf.extend_from_slice(&chunk);
        }

        Ok(bytes::Bytes::from(buf))
    }

    async fn get_token(&self, registry: &str, repository: &str) -> Result<String, ClientError> {
        let scope = format!("repository:{}:pull", repository);

        if registry.contains("docker.io") {
            let token = auth::fetch_token(
                &self.client,
                "https://auth.docker.io/token",
                "registry.docker.io",
                &scope,
                self.credentials.as_ref(),
            )
            .await?;
            return Ok(token);
        }

        // Découvre le realm via WWW-Authenticate
        if let Some((realm, service)) = self.discover_auth(registry).await {
            let token = auth::fetch_token(
                &self.client,
                &realm,
                &service,
                &scope,
                self.credentials.as_ref(),
            )
            .await?;
            return Ok(token);
        }

        // Pas d'auth requis — registry public sans token
        Ok(String::new())
    }

    async fn discover_auth(&self, registry: &str) -> Option<(String, String)> {
        let url = format!("https://{}/v2/", registry);
        let resp = self.client.get(&url).send().await.ok()?;

        let www_auth = resp
            .headers()
            .get("www-authenticate")
            .and_then(|v| v.to_str().ok())?
            .to_string();

        auth::parse_www_authenticate(&www_auth)
    }
}
