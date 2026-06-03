# PolyEdge Frontend

Next.js 16 + React 19 console frontend. Server deployment uses a static export served by Nginx; it does not run `next start` or a Next.js SSR backend.

## Development

```bash
yarn install
yarn dev
```

The dev server runs on [http://localhost:33002](http://localhost:33002).

## Build

```bash
yarn lint
yarn build
```

`yarn build` writes static files to `out/`.

## Docker Runtime

`packages/front/Dockerfile` builds the static export and copies `out/` into an `nginx:1.27-alpine` runtime image.

Runtime settings:

```bash
NEXT_PUBLIC_POLYEDGE_API_BASE_URL=http://192.168.31.5:38001
```

Nginx serves static routes only. Browser API requests go directly to the Rust API base URL above; the current intranet deployment uses `POLYEDGE_AUTH__DISABLED=true` on the API side.
