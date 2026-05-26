# PolyEdge Frontend

Next.js 16 + React 19 console frontend. Server deployment uses a static export served by Nginx; it does not run `next start` or a Next.js SSR backend.

## Development

```bash
pnpm install
pnpm dev
```

The dev server runs on [http://localhost:33002](http://localhost:33002).

## Build

```bash
pnpm lint
pnpm build
```

`pnpm build` writes static files to `out/`.

## Docker Runtime

`packages/front/Dockerfile` builds the static export and copies `out/` into an `nginx:1.27-alpine` runtime image.

Runtime settings:

```bash
POLYEDGE_API_UPSTREAM=http://polyedge-api:38001
POLYEDGE_CONSOLE_STEP_UP_CODE=change-me
```

Nginx serves static routes and proxies `/api/v1/*` to the Rust API.
