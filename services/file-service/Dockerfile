FROM rust:1.75-slim AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services/file-service ./services/file-service

RUN cargo build --release -p file-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/file-service /usr/local/bin/
RUN mkdir -p /data/files
EXPOSE 8082
ENV RUST_LOG=info
ENV STORAGE_PATH=/data/files
CMD ["file-service"]