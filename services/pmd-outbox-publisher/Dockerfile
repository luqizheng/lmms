# Phase 9: standalone Rust binary for PMD outbox publisher.
# Mirrors services/notification-service/Dockerfile, but produces a single
# binary (`pmd-outbox-publisher`) instead of two. CLI process — no HTTP,
# no Swagger UI, no EXPOSE.
#
# Stage 1: rust:1.96-slim-musl builder with workspace-wide dependency cache.
# Stage 2: alpine:3.21 runtime with ca-certificates only.

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
    --mount=type=cache,id=cargo-target-pmd-outbox-publisher,target=/app/target,sharing=locked \
    cargo build --release --target x86_64-unknown-linux-musl --bin pmd-outbox-publisher && \
    cp /app/target/x86_64-unknown-linux-musl/release/pmd-outbox-publisher /app/pmd-outbox-publisher

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/alpine:3.21
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=builder /app/pmd-outbox-publisher /usr/local/bin/pmd-outbox-publisher

# CLI binary — no EXPOSE / no HEALTHCHECK. Process is supervised by
# docker-compose `restart: unless-stopped`. Verification is by a downstream
# consumer reading rows out of pmd_outbox_events.
CMD ["pmd-outbox-publisher"]
