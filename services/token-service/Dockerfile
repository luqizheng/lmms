FROM rust:1.96-slim AS builder
WORKDIR /app

RUN sed -i "s@http://\(deb\|security\).debian.org@https://mirrors.aliyun.com@g" /etc/apt/sources.list.d/debian.sources
RUN apt-get update && apt-get install -y musl-tools pkg-config libssl-dev clang lld mold

RUN mkdir -p /root/.cargo && cat > /root/.cargo/config.toml << 'EOF'
[source.crates-io]
replace-with = 'rsproxy-sparse'
[source.rsproxy]
registry = "https://rsproxy.cn/crates.io-index"
[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
[registries.rsproxy]
index = "https://rsproxy.cn/crates.io-index"
[net]
git-fetch-with-cli = true
[target.x86_64-unknown-linux-musl]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
EOF

COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services
COPY sdk ./sdk
COPY docker/config/token-service docker/config/token-service
COPY docker/secrets docker/secrets

RUN rustup target add x86_64-unknown-linux-musl && \
    CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build --release --target x86_64-unknown-linux-musl -p token-service --offline || \
    CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build --release --target x86_64-unknown-linux-musl -p token-service

FROM alpine:3.21
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/token-service /app/token-service
COPY --from=builder /app/docker/config/token-service/clients.json /app/clients.json
COPY --from=builder /app/docker/secrets/private.pem /app/keys/private.pem
COPY --from=builder /app/docker/secrets/public.pem /app/keys/public.pem
COPY --from=builder /app/services/token-service/config /app/config
ENV RUST_LOG=token_service=debug
CMD ["/app/token-service"]
