FROM zhcoder-docker-registry.com:8000/official/rust:1.96 AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY libs libs
COPY services services
COPY sdk sdk
RUN cargo build --release -p notification-service

FROM zhcoder-docker-registry.com:8000/official/alpine:3.21
RUN apk add --no-cache ca-certificates gcompat
WORKDIR /app
COPY --from=builder /app/target/release/notification-service /usr/local/bin/notification-service
COPY services/notification-service/config /app/config
EXPOSE 8083
CMD ["/usr/local/bin/notification-service", "/app/config/config.yaml"]
