FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
USER root
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services
COPY sdk ./sdk

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    cargo build --release -p audit-service && \
    cp /app/target/release/audit-service /app/audit-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/audit-service /usr/local/bin/
EXPOSE 8080
ENV RUST_LOG=info
CMD ["audit-service"]