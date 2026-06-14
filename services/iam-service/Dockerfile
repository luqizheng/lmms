FROM rust:1.96-slim AS builder

WORKDIR /app

COPY . .

RUN cargo build --release --bin iam-service

FROM alpine:3.21

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/release/iam-service /usr/local/bin/iam-service

COPY services/iam-service/config/config.yaml /etc/iam-service/config.yaml

EXPOSE 8080

CMD ["iam-service"]
