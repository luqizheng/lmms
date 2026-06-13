FROM rust:1.74 AS builder

WORKDIR /app

COPY . .

RUN cargo build --release --bin iam-service

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/iam-service /usr/local/bin/iam-service

COPY services/iam-service/config/config.yaml /etc/iam-service/config.yaml

EXPOSE 8080

CMD ["iam-service"]
