ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official

FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
USER root
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
ENV CARGO_CREDENTIALS_PATH=/app/.cargo/credentials.toml
COPY libs ./libs
COPY services ./services
COPY sdk ./sdk
COPY infrastructure/vendor/swagger-ui-v5.17.14.zip /tmp/swagger-ui.zip

ENV SWAGGER_UI_DOWNLOAD_URL=file:///tmp/swagger-ui.zip

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-target-token-service,target=/app/target,sharing=locked \
    CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build --release --target x86_64-unknown-linux-musl -p token-service && \
    cp /app/target/x86_64-unknown-linux-musl/release/token-service /app/token-service

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/alpine:3.21
RUN apk add --no-cache ca-certificates curl
WORKDIR /app
COPY --from=builder /app/token-service /app/token-service
COPY --from=builder /app/services/token-service/config /app/config
RUN mkdir -p /app/keys
ENV RUST_LOG=token_service=debug

HEALTHCHECK --interval=15s --timeout=3s --retries=5 --start-period=20s \
  CMD curl -f http://127.0.0.1:8082/health || exit 1

CMD ["/app/token-service"]
