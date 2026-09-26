FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/cutman /usr/local/bin/cutman
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
ENV CUTMAN_DATA_DIR=/data \
    CUTMAN_PORT=8080
VOLUME /data
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD curl -fsS "http://127.0.0.1:${CUTMAN_PORT}/health" || exit 1
ENTRYPOINT ["docker-entrypoint.sh"]
