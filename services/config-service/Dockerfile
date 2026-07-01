FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
USER root
WORKDIR /app

ENV SWAGGER_UI_DOWNLOAD_URL=https://gh-proxy.org/https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip

COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY libs ./libs
COPY services ./services
COPY sdk ./sdk

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-target-config-service,target=/app/target,sharing=locked \
    cargo build --release -p config-service && \
    cp /app/target/release/config-service /app/config-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/config-service /usr/local/bin/
EXPOSE 8081
ENV RUST_LOG=info
CMD ["config-service"]
