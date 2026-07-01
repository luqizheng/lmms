ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official

FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder

USER root
WORKDIR /app

RUN apt-get update && apt-get install -y libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*

ENV SWAGGER_UI_DOWNLOAD_URL=https://gh-proxy.org/https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip

COPY . .

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-target-iam-service,target=/app/target,sharing=locked \
    cargo build --release --target x86_64-unknown-linux-musl --bin iam-service -p iam-service && \
    cargo build --release --target x86_64-unknown-linux-musl -p iam-outbox-publisher && \
    cp /app/target/x86_64-unknown-linux-musl/release/iam-service /app/iam-service && \
    cp /app/target/x86_64-unknown-linux-musl/release/iam-outbox-publisher /app/outbox-publisher

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/alpine:3.21

RUN apk add --no-cache ca-certificates curl

COPY --from=builder /app/iam-service /usr/local/bin/iam-service
COPY --from=builder /app/outbox-publisher /usr/local/bin/outbox-publisher

COPY services/iam-service/config/config.yaml /etc/iam-service/config.yaml

EXPOSE 8080

CMD ["iam-service"]
