FROM rust:1.75-slim AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services/config-service ./services/config-service

RUN cargo build --release -p config-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/config-service /usr/local/bin/
EXPOSE 8081
ENV RUST_LOG=info
CMD ["config-service"]
