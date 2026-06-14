# Prebuilt Backend Binaries

The deployment images copy prebuilt binaries from `bin/` directly into runtime
containers. Build them on a Linux environment compatible with the server
runtime, then commit the required binaries to the repository:

```bash
./scripts/build-backend-bin.sh
git add bin/polyedge-api bin/polyedge-orderbook
```

Current Compose deployment uses:

- `bin/polyedge-api` for the API process with embedded worker runtime
- `bin/polyedge-orderbook` for the standalone orderbook service

`polyedge-worker` remains useful as a CLI/maintenance binary, but Docker Compose
does not run it as a separate long-lived service.
