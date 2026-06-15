FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
USER root
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services
COPY sdk ./sdk
COPY docker/config/token-service docker/config/token-service
COPY docker/secrets docker/secrets

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build --release --target x86_64-unknown-linux-musl -p token-service && \
    cp /app/target/x86_64-unknown-linux-musl/release/token-service /app/token-service

FROM alpine:3.21
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=builder /app/token-service /app/token-service
COPY --from=builder /app/docker/config/token-service/clients.json /app/clients.json
COPY --from=builder /app/docker/secrets/private.pem /app/keys/private.pem
COPY --from=builder /app/docker/secrets/public.pem /app/keys/public.pem
COPY --from=builder /app/services/token-service/config /app/config
ENV RUST_LOG=token_service=debug
CMD ["/app/token-service"]
