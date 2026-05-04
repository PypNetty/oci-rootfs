use axum::{
    Json, Router,
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use walkdir::WalkDir;

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
    layers: Vec<String>,
    socket_active: bool,
    upper_size: u64,
}

#[derive(Debug, Serialize)]
struct BlobInfo {
    digest: String,
    size: u64,
    extracted_size: u64,
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

    if auth_header != format!("Bearer {}", token) {
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
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/vms",   get(get_vms))
        .route("/api/blobs", get(get_blobs))
        .route("/api/pull",  post(pull_image))
        .route("/api/pulls", get(get_pulls))
        .nest_service("/", ServeDir::new("frontend/dist"))
        .layer(middleware::from_fn(auth_middleware))
        .layer(cors)
        .with_state(state);

    println!("oci-rootfs-ui → http://localhost:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_vms(State(state): State<AppState>) -> Json<Vec<VmInfo>> {
    let vms_dir = state.store_root.join("vms");
    let mut vms = vec![];

    if let Ok(entries) = std::fs::read_dir(&vms_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let vm_path = entry.path();
            let layers = read_layers(&state.store_root, &vm_path);
            let socket = vm_path.with_extension("sock");
            let socket_active = socket.exists();
            let upper_size = dir_size(&vm_path.join("upper"));
            vms.push(VmInfo { name, layers, socket_active, upper_size });
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
            if name.ends_with("-extracted") { continue; }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let extracted_size = dir_size(&blobs_dir.join(format!("{}-extracted", name)));
            blobs.push(BlobInfo { digest: name, size, extracted_size });
        }
    }

    Json(blobs)
}

async fn pull_image(
    State(state): State<AppState>,
    Json(req): Json<PullRequest>,
) -> Json<serde_json::Value> {
    let name = req.name.clone();
    let image = req.image.clone();

    {
        let mut pulls = state.pulls.lock().await;
        pulls.insert(name.clone(), PullStatus {
            name: name.clone(),
            status: "pulling".to_string(),
            error: None,
        });
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
            Ok(_) => { pulls.insert(name.clone(), PullStatus {
                name, status: "ready".to_string(), error: None,
            }); }
            Err(e) => { pulls.insert(name.clone(), PullStatus {
                name, status: "error".to_string(), error: Some(e.to_string()),
            }); }
        }
    });

    Json(serde_json::json!({ "status": "started", "name": req.name }))
}

async fn get_pulls(State(state): State<AppState>) -> Json<Vec<PullStatus>> {
    let pulls = state.pulls.lock().await;
    Json(pulls.values().cloned().collect())
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

fn read_layers(_store_root: &PathBuf, vm_path: &PathBuf) -> Vec<String> {
    let manifest = vm_path.join("manifest.json");
    if let Ok(content) = std::fs::read_to_string(&manifest) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(layers) = json["layers"].as_array() {
                return layers.iter()
                    .filter_map(|l| l.as_str().map(|s| s[..12].to_string()))
                    .collect();
            }
        }
    }
    vec![]
}