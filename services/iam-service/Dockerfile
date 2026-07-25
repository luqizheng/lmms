ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official

FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder

USER root
WORKDIR /app

RUN apt-get update && apt-get install -y libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*

COPY infrastructure/vendor/swagger-ui-v5.17.14.zip /tmp/swagger-ui.zip
ENV SWAGGER_UI_DOWNLOAD_URL=file:///tmp/swagger-ui.zip

COPY . .

ENV CARGO_CREDENTIALS_PATH=/app/.cargo/credentials.toml

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-target-iam-service,target=/app/target,sharing=locked \
    cargo build --release --target x86_64-unknown-linux-musl --bin iam-service -p iam-service && \
    cargo build --release --target x86_64-unknown-linux-musl -p iam-outbox-publisher && \
    cp /app/target/x86_64-unknown-linux-musl/release/iam-service /app/iam-service && \
    cp /app/target/x86_64-unknown-linux-musl/release/iam-outbox-publisher /app/outbox-publisher

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/alpine:3.21

RUN apk add --no-cache ca-certificates curl

COPY --from=builder /app/iam-service /usr/local/bin/iam-service
COPY --from=builder /app/outbox-publisher /usr/local/bin/outbox-publisher

# Note: no COPY of config.yaml. The default config is embedded in the binary
# via `include_str!("../config/config.default.yaml")` (see src/config.rs) and
# is written to CONFIG_PATH on first boot if the file is absent. Runtime
# overrides come from env vars (see services/iam-service/AGENTS.md).

EXPOSE 8080

HEALTHCHECK --interval=15s --timeout=3s --retries=5 --start-period=20s \
  CMD wget --spider -q http://localhost:8080/health || exit 1

CMD ["iam-service"]
