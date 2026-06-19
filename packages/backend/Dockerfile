FROM debian:trixie-slim AS runtime

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl \
  && rm -rf /var/lib/apt/lists/*

COPY bin/polyedge-api /usr/local/bin/polyedge-api
COPY bin/polyedge-orderbook /usr/local/bin/polyedge-orderbook
RUN chmod 0755 /usr/local/bin/polyedge-api /usr/local/bin/polyedge-orderbook

ENV POLYEDGE_SERVER__HOST=0.0.0.0
ENV POLYEDGE_SERVER__PORT=38001

EXPOSE 38001
CMD ["polyedge-api"]
