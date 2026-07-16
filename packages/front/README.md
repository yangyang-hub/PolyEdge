# PolyEdge Frontend

Next.js 16.2 + React 19.2 static console for session login/activation, dashboard, strategies, following, encrypted wallets, operations, settings, and administrator views. Nginx serves the exported files and proxies same-origin `/api/*` traffic to the V4 `polyedge-server`.

## Local development

```bash
yarn install
yarn dev
```

Leave `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` empty for the production same-origin Nginx proxy. A separate backend origin is supported only for local development with matching CORS/Origin configuration. Wallet plaintext exists only briefly in the browser before WebCrypto envelope upload and is never persisted by the frontend.

## Validation

```bash
npx tsc --noEmit
yarn lint
yarn build
```

The product flow is Cookie-session authentication, administrator-created users, time-bounded manual strategies, cross-user strategy following with follower-owned wallets, browser-encrypted wallet import, managed order/position inspection, and protected kill-switch control. Market scanning, Funding, events/news, AI/info filtering, and fair value are not frontend capabilities.
