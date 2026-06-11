FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
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
RUN apt-get update && apt-get install -y clang lld mold 2>&1 | tail -2 || true

COPY Cargo.toml Cargo.lock ./
COPY libs/ libs/
COPY services/ services/
COPY sdk sdk
RUN cargo build --release --target x86_64-unknown-linux-musl -p api-gateway

FROM zhcoder-docker-registry.com:8000/official/alpine:3.21
WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/api-gateway /app/api-gateway
COPY docker/config/api-gateway /app/config/api-gateway
ENV RUST_LOG=api_gateway=debug
CMD ["/app/api-gateway", "serve", "--routes", "/app/config/api-gateway/routes"]
