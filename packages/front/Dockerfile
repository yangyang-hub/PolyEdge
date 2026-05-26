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
RUN mkdir -p public
RUN pnpm build

FROM nginx:1.27-alpine AS runtime

ENV POLYEDGE_API_UPSTREAM=http://polyedge-api:38001
ENV POLYEDGE_CONSOLE_STEP_UP_CODE=__unset__

COPY packages/front/nginx.conf.template /etc/nginx/templates/default.conf.template
COPY --from=builder /app/packages/front/out /usr/share/nginx/html

EXPOSE 33002
CMD ["sh", "-c", "test -n \"$POLYEDGE_CONSOLE_STEP_UP_CODE\" && test \"$POLYEDGE_CONSOLE_STEP_UP_CODE\" != \"__unset__\" || { echo 'POLYEDGE_CONSOLE_STEP_UP_CODE is required for the static frontend proxy.' >&2; exit 1; }; nginx -g 'daemon off;'"]
