# PolyEdge Frontend

Last updated: 2026-07-12

Next.js 16.2 + React 19.2 console frontend for `dashboard / markets / events / rewards / rewards/fair-value / funding / settings`. Server deployment uses a static export served by Nginx; it does not run `next start` or a Next.js SSR backend. The UI reads the real Rust API only; mock-data mode has been removed.

## Development

```bash
yarn install
yarn dev
```

The dev server runs on [http://localhost:33002](http://localhost:33002).

## Build

```bash
npx tsc --noEmit
yarn lint
yarn build
```

`yarn build` writes static files to `out/`.

## Docker Runtime

`yarn build` writes the static export to `out/`. `packages/front/Dockerfile` only copies that prebuilt `out/` directory into an `nginx:1.27-alpine` runtime image.

Runtime settings:

```bash
NEXT_PUBLIC_POLYEDGE_API_BASE_URL=http://100.87.45.72:38001
NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH=off
```

Current production troubleshooting URLs: Front Rewards at `http://192.168.31.5:33002/rewards`, API at `http://100.87.45.72:38001`, and orderbook at `http://100.87.45.72:38002`.

Nginx serves static routes only. Browser API requests go directly to the Rust API base URL above; the current intranet deployment uses `POLYEDGE_AUTH__DISABLED=true` on the API side. Static assets and HTML are served with `Cache-Control: no-cache, must-revalidate`; `scripts/deploy.sh` also appends a front hash query to exported `/_next/static/*.js/css` references so dashboard updates are not hidden behind stale chunks.

`NEXT_PUBLIC_*` values are compiled into the browser bundle, so changing the API URL or console mode requires rebuilding the static export. The only supported console auth mode is currently `off`; production usability therefore depends on a VPN/private ACL or trusted reverse proxy boundary around the API. CORS is not authentication.
