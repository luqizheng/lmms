FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
USER root
WORKDIR /app

# 阶段1: 只复制依赖文件，构建依赖层（利用 Docker 缓存）
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services
COPY sdk ./sdk

# 预编译依赖（仅当 Cargo.toml/Cargo.lock 变化时重新执行）
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    cargo build --release --target x86_64-unknown-linux-musl -p api-gateway --jobs 8 && \
    cp /app/target/x86_64-unknown-linux-musl/release/api-gateway /app/api-gateway

# 阶段2: 复制源代码并编译
COPY services/api-gateway/src ./services/api-gateway/src

FROM alpine:3.21
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=builder /app/api-gateway /app/api-gateway
COPY docker-example/config/api-gateway/routes /app/config/routes
ENV RUST_LOG=api_gateway=debug
CMD ["/app/api-gateway", "serve", "--routes", "/app/config/routes"]
