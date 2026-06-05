FROM debian:trixie-slim AS runtime

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl \
  && rm -rf /var/lib/apt/lists/*

COPY polyedge-worker /usr/local/bin/polyedge-worker
RUN chmod 0755 /usr/local/bin/polyedge-worker

ENV POLYEDGE_SERVER__HOST=0.0.0.0
ENV POLYEDGE_SERVER__PORT=38001

EXPOSE 38001
CMD ["polyedge-worker"]
