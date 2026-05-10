FROM debian:bookworm-slim AS runtime

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl \
  && rm -rf /var/lib/apt/lists/*

COPY bin/polyedge-api /usr/local/bin/polyedge-api
RUN chmod 0755 /usr/local/bin/polyedge-api

ENV POLYEDGE_SERVER__HOST=0.0.0.0
ENV POLYEDGE_SERVER__PORT=8080

EXPOSE 8080
CMD ["polyedge-api"]
