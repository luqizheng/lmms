FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services
COPY sdk sdk

RUN cargo build --release -p api-gateway

FROM alpine:3.21
WORKDIR /app
COPY --from=builder /app/target/release/api-gateway /app/api-gateway
COPY docker/config/api-gateway/routes /app/config/routes
ENV RUST_LOG=api_gateway=debug
CMD ["/app/api-gateway", "serve", "--routes", "/app/config/routes"]
