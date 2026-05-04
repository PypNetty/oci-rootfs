# oci-rootfs

Rust library that converts any OCI image into a rootfs ready for Firecracker.

## What it does



debian:trixie-slim → fuse-overlayfs mount → virtiofsd socket node:22-slim → 6 layers, deduplicated, assembled ghcr.io/owner/repo → any OCI-compatible registry


No Docker. No containerd. No root.

## How it works

1. Pull manifest from registry (Docker Hub, ghcr.io, quay.io, private)
2. Download and verify each layer by SHA256 digest
3. Extract layers into a content-addressable cache (`~/.local/share/kyernal/blobs/sha256/`)
4. Mount layers via `fuse-overlayfs` (rootless overlay)
5. Serve the merged rootfs via `virtiofsd` (vhost-user socket)

## Usage

```rust
use oci_rootfs::{RootfsBuilder, registry::manifest::Arch};

let rootfs = RootfsBuilder::new()
    .image("node:22-slim")
    .name("sandbox-01")
    .arch(Arch::Amd64)
    .build()
    .await?;

// rootfs.merged  → merged directory (6 layers assembled)
// rootfs.socket  → vhost-user socket for Firecracker


Store layout

~/.local/share/kyernal/
├── blobs/sha256/          # layers, deduplicated across all VMs
│   ├── 3531af2b...        # debian base layer (shared)
│   └── d0513e5e...        # node binary layer
└── vms/
    └── sandbox-01/
        ├── upper/         # VM writes (isolated per VM)
        ├── work/          # overlayfs workdir
        └── merged/        # complete rootfs view


Requirements





fuse-overlayfs



virtiofsd (/usr/libexec/virtiofsd or /usr/lib/qemu/virtiofsd)

Status

Working. Validated with debian:trixie-slim (1 layer) and node:22-slim (6 layers). Part of the Kyernal project.

License

Apache-2.0






