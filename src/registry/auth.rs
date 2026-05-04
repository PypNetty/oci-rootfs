// src/registry/auth.rs

use reqwest::Client;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("no token in response")]
    NoToken,
}

/// Credentials optionnelles pour registries privés
#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: Option<String>,
    access_token: Option<String>,
}

/// Récupère un token Bearer pour un registry/repo donné
/// Gère Docker Hub, ghcr.io, quay.io — tous suivent le même flow WWW-Authenticate
pub async fn fetch_token(
    client: &Client,
    realm: &str,
    service: &str,
    scope: &str,
    credentials: Option<&Credentials>,
) -> Result<String, AuthError> {
    let mut req = client
        .get(realm)
        .query(&[("service", service), ("scope", scope)]);

    if let Some(creds) = credentials {
        req = req.basic_auth(&creds.username, Some(&creds.password));
    }

    let resp: TokenResponse = req.send().await?.json().await?;

    resp.token
        .or(resp.access_token)
        .ok_or(AuthError::NoToken)
}

/// Parse le header WWW-Authenticate pour extraire realm, service, scope
/// Exemple : Bearer realm="https://auth.docker.io/token",service="registry.docker.io"
pub fn parse_www_authenticate(header: &str) -> Option<(String, String)> {
    let realm = extract_value(header, "realm")?;
    let service = extract_value(header, "service").unwrap_or_default();
    Some((realm, service))
}

fn extract_value(header: &str, key: &str) -> Option<String> {
    let needle = format!("{}=\"", key);
    let start = header.find(&needle)? + needle.len();
    let end = header[start..].find('"')? + start;
    Some(header[start..end].to_string())
}