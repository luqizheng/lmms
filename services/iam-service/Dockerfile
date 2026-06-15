FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder

USER root
WORKDIR /app

COPY . .

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    cargo build --release --bin iam-service && \
    cp /app/target/release/iam-service /app/iam-service

FROM alpine:3.21

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/iam-service /usr/local/bin/iam-service

COPY services/iam-service/config/config.yaml /etc/iam-service/config.yaml

EXPOSE 8080

CMD ["iam-service"]
