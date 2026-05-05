// src/registry/auth.rs

use reqwest::Client;
use serde::Deserialize;
use thiserror::Error;
use zeroize::Zeroize;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("no token in response")]
    NoToken,
    #[error("parse error: {0} — body: {1}")]
    Parse(String, String),
}

#[derive(Debug, Default)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

impl Drop for Credentials {
    fn drop(&mut self) {
        self.username.zeroize();
        self.password.zeroize();
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
}

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

    let resp = req.send().await?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| {
        AuthError::Parse(format!("status={} text_error={}", status, e), String::new())
    })?;

    let parsed: TokenResponse =
        serde_json::from_str(&text).map_err(|e| AuthError::Parse(e.to_string(), text.clone()))?;

    parsed
        .token
        .or(parsed.access_token)
        .ok_or(AuthError::NoToken)
}

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
