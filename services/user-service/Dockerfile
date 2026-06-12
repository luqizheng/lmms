FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
RUN export RUSTUP_DIST_SERVER="https://rsproxy.cn"
RUN export RUSTUP_UPDATE_ROOT="https://rsproxy.cn/rustup"
RUN curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh | sh

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
RUN cargo build --release --target x86_64-unknown-linux-musl -p user-service

FROM zhcoder-docker-registry.com:8000/official/alpine:3.21
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/user-service /usr/local/bin/user-service
COPY services/user-service/config /app/config
EXPOSE 8080
CMD ["user-service"]
