FROM node:20-bookworm-slim AS deps

ENV NEXT_TELEMETRY_DISABLED=1
WORKDIR /app/packages/front

RUN corepack enable
COPY packages/front/package.json packages/front/pnpm-lock.yaml packages/front/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

FROM node:20-bookworm-slim AS builder

ENV NEXT_TELEMETRY_DISABLED=1
WORKDIR /app/packages/front

RUN corepack enable
COPY --from=deps /app/packages/front/node_modules ./node_modules
COPY packages/front ./
RUN pnpm build

FROM node:20-bookworm-slim AS runtime

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV HOSTNAME=0.0.0.0
ENV PORT=33001

WORKDIR /app

COPY --from=builder /app/packages/front/.next/standalone ./
COPY --from=builder /app/packages/front/.next/static ./.next/static
COPY --from=builder /app/packages/front/public ./public

EXPOSE 33001
CMD ["node", "server.js"]
