# Prebuilt Backend Binaries

The deployment image copies `bin/polyedge-api` directly into the backend
container. Build this binary on a Linux environment compatible with the server
runtime, then commit it to the repository:

```bash
./scripts/build-backend-bin.sh
git add bin/polyedge-api
```

The server-side deploy script only rebuilds the backend image when this binary
or backend deployment files change.
