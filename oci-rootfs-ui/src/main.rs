use axum::{
    Json, Router,
    extract::{Request, State},
    http::{Method, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use subtle::ConstantTimeEq;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use walkdir::WalkDir;

fn is_valid_digest(digest: &str) -> bool {
    digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains("..")
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        && name.len() <= 128
}

#[derive(Clone)]
struct AppState {
    store_root: PathBuf,
    pulls: Arc<Mutex<std::collections::HashMap<String, PullStatus>>>,
}

#[derive(Debug, Clone, Serialize)]
struct PullStatus {
    name: String,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct VmInfo {
    name: String,
    image: String,
    layers: Vec<String>,
    layers_downloaded: u32,
    layers_cached: u32,
    bytes_downloaded: u64,
    bytes_total: u64,
    pull_duration_ms: u64,
    pulled_at: String,
    socket_active: bool,
    upper_size: u64,
}

#[derive(Debug, Serialize)]
struct BlobInfo {
    digest: String,
    size: u64,
    extracted_size: u64,
    duration_ms: u64,
    pulled_at: String,
    expires_at: String,
    ttl_hours: u64,
}

#[derive(Debug, Deserialize)]
struct PullRequest {
    image: String,
    name: String,
}

async fn auth_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    if !request.uri().path().starts_with("/api") {
        return Ok(next.run(request).await);
    }

    let token = std::env::var("KYERNAL_TOKEN").unwrap_or_default();
    if token.is_empty() {
        return Ok(next.run(request).await);
    }

    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let expected = format!("Bearer {}", token);
    if auth_header.len() != expected.len() ||
       auth_header.as_bytes().ct_eq(expected.as_bytes()).unwrap_u8() != 1 {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

#[tokio::main]
async fn main() {
    let store_root = dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("/var/lib/kyernal"))
        .join("kyernal");

    let state = AppState {
        store_root,
        pulls: Arc::new(Mutex::new(std::collections::HashMap::new())),
    };

    let cors = CorsLayer::new()
        .allow_origin("http://localhost:3000".parse::<axum::http::HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    let app = Router::new()
        .route("/api/vms", get(get_vms))
        .route("/api/vms/:name", delete(delete_vm))
        .route("/api/blobs", get(get_blobs))
        .route("/api/blobs/:digest", delete(delete_blob))
        .route("/api/pull", post(pull_image))
        .route("/api/test-delete", delete(|| async { "ok" }))
        .route("/api/pulls", get(get_pulls))
        .with_state(state)
        .fallback_service(ServeDir::new("frontend/dist"))
        .layer(middleware::from_fn(auth_middleware))
        .layer(cors);

    let token = std::env::var("KYERNAL_TOKEN").unwrap_or_default();
    let bind_addr = std::env::var("KYERNAL_UI_BIND")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    // Require token when binding to 0.0.0.0 or [::] (all interfaces)
    let bind_requires_auth = bind_addr.starts_with("0.0.0.0") || bind_addr == "[::]";
    if bind_requires_auth && token.is_empty() {
        eprintln!("Error: KYERNAL_TOKEN must be set when binding to 0.0.0.0 or [::]");
        std::process::exit(1);
    }

    println!("oci-rootfs-ui → http://{}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_vms(State(state): State<AppState>) -> Json<Vec<VmInfo>> {
    let vms_dir = state.store_root.join("vms");
    let mut vms = vec![];

    if let Ok(entries) = std::fs::read_dir(&vms_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let vm_path = entry.path();
            let info = read_vm_info(&vm_path).unwrap_or_default();
            let socket = vm_path.with_extension("sock");

            vms.push(VmInfo {
                name,
                image: info["image"].as_str().unwrap_or("unknown").to_string(),
                layers: info["layers"]
                    .as_array()
                    .map(|l| {
                        l.iter()
                            .filter_map(|v| v.as_str().map(|s| s[..12.min(s.len())].to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
                layers_downloaded: info["layers_downloaded"].as_u64().unwrap_or(0) as u32,
                layers_cached: info["layers_cached"].as_u64().unwrap_or(0) as u32,
                bytes_downloaded: info["bytes_downloaded"].as_u64().unwrap_or(0),
                bytes_total: info["bytes_total"].as_u64().unwrap_or(0),
                pull_duration_ms: info["pull_duration_ms"].as_u64().unwrap_or(0),
                pulled_at: info["pulled_at"].as_str().unwrap_or("").to_string(),
                socket_active: socket.exists(),
                upper_size: dir_size(&vm_path.join("upper")),
            });
        }
    }

    Json(vms)
}

async fn get_blobs(State(state): State<AppState>) -> Json<Vec<BlobInfo>> {
    let blobs_dir = state.store_root.join("blobs/sha256");
    let mut blobs = vec![];

    if let Ok(entries) = std::fs::read_dir(&blobs_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with("-extracted") || name.ends_with("-meta.json") {
                continue;
            }

            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let extracted_size = dir_size(&blobs_dir.join(format!("{}-extracted", name)));
            let meta = read_blob_meta(&blobs_dir.join(format!("{}-meta.json", name)));

            blobs.push(BlobInfo {
                digest: name,
                size,
                extracted_size,
                duration_ms: meta["duration_ms"].as_u64().unwrap_or(0),
                pulled_at: meta["pulled_at"].as_str().unwrap_or("").to_string(),
                expires_at: meta["expires_at"].as_str().unwrap_or("").to_string(),
                ttl_hours: meta["ttl_hours"].as_u64().unwrap_or(0),
            });
        }
    }

    Json(blobs)
}

async fn pull_image(
    State(state): State<AppState>,
    Json(req): Json<PullRequest>,
) -> Json<serde_json::Value> {
    if !is_safe_name(&req.name) {
        return Json(serde_json::json!({ "error": "invalid name" }));
    }

    let name = req.name.clone();
    let image = req.image.clone();

    {
        let mut pulls = state.pulls.lock().await;
        pulls.insert(
            name.clone(),
            PullStatus {
                name: name.clone(),
                status: "pulling".to_string(),
                error: None,
            },
        );
    }

    let pulls = state.pulls.clone();
    tokio::spawn(async move {
        let result = oci_rootfs::RootfsBuilder::new()
            .image(&image)
            .name(&name)
            .build()
            .await;

        let mut pulls = pulls.lock().await;
        match result {
            Ok(_) => {
                pulls.insert(
                    name.clone(),
                    PullStatus {
                        name,
                        status: "ready".to_string(),
                        error: None,
                    },
                );
            }
            Err(e) => {
                pulls.insert(
                    name.clone(),
                    PullStatus {
                        name,
                        status: "error".to_string(),
                        error: Some(e.to_string()),
                    },
                );
            }
        }
    });

    Json(serde_json::json!({ "status": "started", "name": req.name }))
}

async fn get_pulls(State(state): State<AppState>) -> Json<Vec<PullStatus>> {
    let pulls = state.pulls.lock().await;
    Json(pulls.values().cloned().collect())
}

async fn delete_vm(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !is_safe_name(&name) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let vm_path = state.store_root.join("vms").join(&name);
    if !vm_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let merged = vm_path.join("merged");
    let _ = std::process::Command::new("fusermount3")
        .args(["-u", merged.to_str().unwrap_or("")])
        .status();

    std::fs::remove_dir_all(&vm_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "deleted": name })))
}

async fn delete_blob(
    State(state): State<AppState>,
    digest: axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let digest = if digest.starts_with("sha256:") {
        &digest[7..]
    } else {
        &digest
    };

    if !is_valid_digest(digest) {
        return Err((StatusCode::BAD_REQUEST, "invalid digest format".to_string()));
    }

    let blobs_dir = state.store_root.join("blobs/sha256");
    let blob = blobs_dir.join(digest);
    let extracted = blobs_dir.join(format!("{}-extracted", digest));
    let meta = blobs_dir.join(format!("{}-meta.json", digest));

    if !blob.exists() {
        return Err((StatusCode::NOT_FOUND, format!("blob not found: {}", digest)));
    }

    let _ = std::fs::remove_file(&blob);
    let _ = std::fs::remove_dir_all(&extracted);
    let _ = std::fs::remove_file(&meta);

    Ok(Json(serde_json::json!({ "deleted": digest })))
}

fn dir_size(path: &PathBuf) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

fn read_vm_info(vm_path: &PathBuf) -> Option<serde_json::Value> {
    let manifest = vm_path.join("manifest.json");
    let content = std::fs::read_to_string(&manifest).ok()?;
    serde_json::from_str(&content).ok()
}

fn read_blob_meta(path: &PathBuf) -> serde_json::Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or(serde_json::Value::Null)
}
