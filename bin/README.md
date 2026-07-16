# Prebuilt Backend Binary

Last updated: 2026-07-15

V3 deployment uses one Rust artifact only:

- `bin/polyedge-server`

Build it on a Linux environment compatible with the target server:

```bash
./scripts/build-backend-bin.sh
git add bin/polyedge-server
```

The script runs `cargo build --release -p polyedge-server`, copies the result to this directory, makes it executable, and prints its SHA-256.

Legacy `polyedge-api`, `polyedge-orderbook`, `polyedge-worker`, and `polyedge-replay` binaries are not V3 deployment artifacts and must not be referenced by Docker Compose or deployment scripts.
