ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official

FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
USER root
WORKDIR /app

COPY infrastructure/vendor/swagger-ui-v5.17.14.zip /tmp/swagger-ui.zip
ENV SWAGGER_UI_DOWNLOAD_URL=file:///tmp/swagger-ui.zip

COPY . .

ENV CARGO_CREDENTIALS_PATH=/app/.cargo/credentials.toml
# 方言开关: mysql(默认,与 default features 等价) | postgres(发布 PG 变体时由 --build-arg 传入)
ARG SERVICE_FEATURES=mysql

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-target-file-service,target=/app/target,sharing=locked \
    cargo build --release --target x86_64-unknown-linux-musl -p file_service  --features "${SERVICE_FEATURES}" && \
    cp /app/target/x86_64-unknown-linux-musl/release/file_service /app/file_service

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/alpine:3.21

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/file_service /usr/local/bin/

RUN mkdir -p /data/files
EXPOSE 8082

HEALTHCHECK --interval=15s --timeout=3s --retries=5 --start-period=20s \
  CMD wget --spider -q http://127.0.0.1:8082/health || exit 1

ENV RUST_LOG=info
ENV STORAGE_PATH=/data/files
CMD ["file_service"]
