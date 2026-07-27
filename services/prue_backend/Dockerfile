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
    --mount=type=cache,id=cargo-target-prue-backend,target=/app/target,sharing=locked \
    cargo build --release --target x86_64-unknown-linux-musl --bin prue_backend && \
    cp /app/target/x86_64-unknown-linux-musl/release/prue_backend /app/prue_backend

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/alpine:3.21

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/prue_backend /usr/local/bin/prue_backend

EXPOSE 8087

HEALTHCHECK --interval=15s --timeout=3s --retries=5 --start-period=20s \
  CMD wget --spider -q http://127.0.0.1:8087/health || exit 1

CMD ["prue_backend"]
