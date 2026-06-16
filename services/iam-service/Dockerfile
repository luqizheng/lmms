FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder

USER root
WORKDIR /app

RUN apt-get update && apt-get install -y libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*

COPY . .

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    cargo build --release --target x86_64-unknown-linux-musl --bin iam-service && \
    cp /app/target/x86_64-unknown-linux-musl/release/iam-service /app/iam-service

FROM alpine:3.21

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/iam-service /usr/local/bin/iam-service

COPY services/iam-service/config/config.yaml /etc/iam-service/config.yaml

RUN mkdir -p /etc/iam-service/keys

COPY docker/secrets/private.pem /etc/iam-service/keys/private.pem
COPY docker/secrets/public.pem /etc/iam-service/keys/public.pem

EXPOSE 8080

CMD ["iam-service"]
