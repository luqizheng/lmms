FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
USER root
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY libs/ libs/
COPY services/ services/
COPY sdk sdk

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    export SWAGGER_UI_DOWNLOAD_URL=https://gh-proxy.org/https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip && \
    cargo build --release --target x86_64-unknown-linux-musl --bin notification-service --bin ntf-outbox-publisher && \
    cp /app/target/x86_64-unknown-linux-musl/release/notification-service /app/notification-service && \
    cp /app/target/x86_64-unknown-linux-musl/release/ntf-outbox-publisher /app/ntf-outbox-publisher

FROM alpine:3.21
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=builder /app/notification-service /usr/local/bin/notification-service
COPY --from=builder /app/ntf-outbox-publisher /usr/local/bin/ntf-outbox-publisher
COPY services/notification-service/config /app/config
EXPOSE 8083
CMD ["notification-service", "/app/config/config.yaml"]
