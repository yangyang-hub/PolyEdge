# PolyEdge Frontend

Next.js 16.2 + React 19.2 static console for `/dashboard`, `/strategies`, `/wallets`, `/operations`, and `/settings`. Nginx serves the exported files; the browser talks directly to the V3 `polyedge-server` HTTP API.

## Local development

```bash
yarn install
yarn dev
```

Configure `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` with the backend origin. The frontend has no mock-data mode and does not contain wallet secrets.

## Validation

```bash
npx tsc --noEmit
yarn lint
yarn build
```

The product flow is manual market strategy entry, credential-reference wallet setup, one-strategy-to-many-wallet execution, managed order/position inspection, and protected kill-switch control. Market scanning, Funding, events/news, AI/info filtering, and fair value are not frontend capabilities.
