ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official

FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
USER root
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY libs/ libs/
COPY services/ services/
COPY sdk sdk

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-target-audit-service,target=/app/target,sharing=locked \
    SWAGGER_UI_DOWNLOAD_URL=https://gh-proxy.org/https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip \
    cargo build --release --target x86_64-unknown-linux-musl -p audit-service && \
    cp /app/target/x86_64-unknown-linux-musl/release/audit-service /app/audit-service

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/alpine:3.21
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/audit-service /usr/local/bin/audit-service
EXPOSE 8080
ENV RUST_LOG=info
CMD ["audit-service"]
