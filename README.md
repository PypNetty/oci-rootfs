# oci-rootfs

Rust library that converts any OCI image into a rootfs ready for Firecracker.

No Docker. No containerd. No root.

---

## What it does

```
debian:trixie-slim   →  1 layer   →  28.4 MB  →  75.0 MB extracted  →  virtiofsd socket
node:22-slim         →  5 layers  →  75.9 MB  →  289.0 MB extracted  →  virtiofsd socket
node:24.15.0         →  8 layers  →  cached   →  shared base layer   →  virtiofsd socket
ghcr.io/owner/repo   →  any OCI-compatible registry
```

Layers are deduplicated across VMs. Pull `debian:trixie-slim` once — every image built on top shares the same base blob.

## How it works

1. Pull manifest from registry (Docker Hub, ghcr.io, quay.io, private)
2. Select the right manifest for the target arch (amd64 / arm64)
3. Download and verify each layer by SHA256 digest
4. Extract layers into a content-addressable cache
5. Mount layers via `fuse-overlayfs` (rootless overlay)
6. Serve the merged rootfs via `virtiofsd` (vhost-user socket)
7. Save pull metrics — bytes downloaded, duration per layer, TTL

## Usage

```rust
use oci_rootfs::{RootfsBuilder, registry::manifest::Arch};

let rootfs = RootfsBuilder::new()
    .image("node:22-slim")
    .name("sandbox-01")
    .arch(Arch::Amd64)
    .ttl_hours(24)
    .build()
    .await?;

// rootfs.merged  → merged directory, ready for Firecracker virtio-fs
// rootfs.socket  → vhost-user socket
```

## Store layout

```
~/.local/share/kyernal/
├── blobs/sha256/
│   ├── 3531af2b...              # layer blob (tar.gz)
│   ├── 3531af2b...-extracted/   # extracted filesystem
│   └── 3531af2b...-meta.json    # pulled_at, duration_ms, expires_at, ttl_hours
└── vms/
    └── sandbox-01/
        ├── manifest.json        # image, layers, pull metrics
        ├── upper/               # VM writes (isolated per VM)
        ├── work/                # overlayfs workdir
        └── merged/              # complete rootfs view → virtiofsd
```

## Security

- Path traversal protection on tar extraction
- SHA256 digest verification on every layer before extraction
- 100 MB max per layer, 300 MB max rootfs total
- Rootless — no uid=0 process on the host
- Optional API token via `KYERNAL_TOKEN` env var

## Dashboard

`oci-rootfs-ui` ships a local web dashboard (Axum + React) for inspecting the store:

- Live VM list with pull metrics per VM
- Blob cache with per-layer download time, size, TTL and expiry
- Pull any image directly from the UI
- Delete VMs and blobs

```bash
cd oci-rootfs-ui && cargo run
# → http://localhost:3000
```

## Requirements

- `fuse-overlayfs`
- `virtiofsd` (`/usr/libexec/virtiofsd` or `/usr/lib/qemu/virtiofsd`)

## Status

Pre-alpha. Core pipeline working.

Validated with:
- `debian:trixie-slim` — 1 layer, 28.4 MB, boots in 2.81s cold
- `node:22-slim` — 5 layers, 75.9 MB, deduplicated base layer
- `node:24.15.0` — 8 layers, shared debian base

Part of the [Kyernal](https://github.com/PypNetty/kyernal) project — the Proxmox of microVMs.

## License

Apache-2.0