ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official

FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
USER root
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY libs ./libs
COPY services ./services
COPY sdk ./sdk
COPY docker-example/config/token-service docker-example/config/token-service
COPY docker-example/secrets docker-example/secrets

ENV SWAGGER_UI_DOWNLOAD_URL=https://gh-proxy.org/https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip

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
COPY --from=builder /app/docker-example/config/token-service/clients.json /app/clients.json
COPY --from=builder /app/docker-example/secrets/private.pem /app/keys/private.pem
COPY --from=builder /app/docker-example/secrets/public.pem /app/keys/public.pem
COPY --from=builder /app/services/token-service/config /app/config
ENV RUST_LOG=token_service=debug
CMD ["/app/token-service"]
