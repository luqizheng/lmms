FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
USER root
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY libs/ libs/
COPY services/ services/
COPY sdk sdk
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    cargo build --release --target x86_64-unknown-linux-musl -p notification-service && \
    cp /app/target/x86_64-unknown-linux-musl/release/notification-service /app/notification-service

FROM alpine:3.21
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=builder /app/notification-service /usr/local/bin/notification-service
COPY services/notification-service/config /app/config
EXPOSE 8083
CMD ["notification-service", "/app/config/config.yaml"]
