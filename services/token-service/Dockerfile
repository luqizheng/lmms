FROM zhcoder-docker-registry.com:8000/official/rust:1.96-slim AS chef
RUN sed -i "s@http://\(deb\|security\).debian.org@https://mirrors.aliyun.com@g" /etc/apt/sources.list.d/debian.sources
RUN apt update && apt install -y musl-tools pkg-config libssl-dev mold clang
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY libs/ libs/
COPY services/ services/
COPY sdk sdk
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
RUN rustup target add x86_64-unknown-linux-musl
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
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl -p token-service
COPY Cargo.toml Cargo.lock ./
COPY libs/ libs/
COPY services/ services/
COPY sdk sdk
RUN cargo build --release --target x86_64-unknown-linux-musl -p token-service

FROM zhcoder-docker-registry.com:8000/official/alpine:3.21
WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/token-service /app/token-service
COPY docker/config/token-service/clients.json /app/clients.json
COPY docker/secrets/private.pem /app/keys/private.pem
COPY docker/secrets/public.pem /app/keys/public.pem
COPY services/token-service/config /app/config
ENV RUST_LOG=token_service=debug
CMD ["/app/token-service"]
