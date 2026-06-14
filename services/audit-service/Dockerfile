FROM rust:1.75-slim AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services/audit-service ./services/audit-service

RUN cargo build --release -p audit-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/audit-service /usr/local/bin/
EXPOSE 8080
ENV RUST_LOG=info
CMD ["audit-service"]
