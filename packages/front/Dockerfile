FROM node:20-bookworm-slim AS deps

ENV NEXT_TELEMETRY_DISABLED=1
WORKDIR /app/packages/front

RUN corepack enable
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

FROM node:20-bookworm-slim AS builder

ENV NEXT_TELEMETRY_DISABLED=1
WORKDIR /app/packages/front

RUN corepack enable
COPY --from=deps /app/packages/front/node_modules ./node_modules
COPY . ./
RUN mkdir -p public
RUN pnpm build

FROM nginx:1.27-alpine AS runtime

ENV POLYEDGE_API_UPSTREAM=http://polyedge-api:38001
ENV POLYEDGE_CONSOLE_STEP_UP_CODE=__unset__

COPY nginx.conf.template /etc/nginx/polyedge.conf.template
COPY --from=builder /app/packages/front/out /usr/share/nginx/html
RUN rm -f /etc/nginx/conf.d/default.conf

EXPOSE 80
CMD ["sh", "-c", "test -n \"$POLYEDGE_CONSOLE_STEP_UP_CODE\" && test \"$POLYEDGE_CONSOLE_STEP_UP_CODE\" != \"__unset__\" || { echo 'POLYEDGE_CONSOLE_STEP_UP_CODE is required for the static frontend proxy.' >&2; exit 1; }; envsubst '$${POLYEDGE_API_UPSTREAM} $${POLYEDGE_CONSOLE_STEP_UP_CODE}' < /etc/nginx/polyedge.conf.template > /etc/nginx/conf.d/polyedge.conf && nginx -g 'daemon off;'"]
