FROM rust:1.81-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY libs libs
COPY services services
RUN cargo build --release -p user-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/user-service /usr/local/bin/user-service
COPY services/user-service/config /app/config
EXPOSE 8082
CMD ["user-service"]
