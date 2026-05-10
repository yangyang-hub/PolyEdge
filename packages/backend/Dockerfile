ARG RUST_VERSION=1.93.0

FROM rust:${RUST_VERSION}-bookworm AS builder

WORKDIR /app
COPY packages/backend ./packages/backend

WORKDIR /app/packages/backend
ARG PACKAGE=polyedge-api
RUN cargo build --release -p "${PACKAGE}"

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl \
  && rm -rf /var/lib/apt/lists/*

ARG BIN=polyedge-api
COPY --from=builder /app/packages/backend/target/release/${BIN} /usr/local/bin/${BIN}

ENV POLYEDGE_SERVER__HOST=0.0.0.0
ENV POLYEDGE_SERVER__PORT=8080

EXPOSE 8080
CMD ["polyedge-api"]
