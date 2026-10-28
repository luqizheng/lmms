ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official

FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder
USER root
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
ENV CARGO_CREDENTIALS_PATH=/app/.cargo/credentials.toml
COPY libs/ libs/
COPY services/ services/
COPY sdk sdk
COPY infrastructure/vendor/swagger-ui-v5.17.14.zip /tmp/swagger-ui.zip

ENV SWAGGER_UI_DOWNLOAD_URL=file:///tmp/swagger-ui.zip

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-target-notification-service,target=/app/target,sharing=locked \
    cargo build --release --target x86_64-unknown-linux-musl --bin notification_service --bin ntf_outbox_publisher && \
    cp /app/target/x86_64-unknown-linux-musl/release/notification_service /app/notification_service && \
    cp /app/target/x86_64-unknown-linux-musl/release/ntf_outbox_publisher /app/ntf_outbox_publisher

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/alpine:3.21
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=builder /app/notification_service /usr/local/bin/notification_service
COPY --from=builder /app/ntf_outbox_publisher /usr/local/bin/ntf_outbox_publisher

# 角色感知健康探针：这张镜像同时承载服务端（notification_service，8083）与 worker
# （ntf_outbox_publisher，无端口），单一 HEALTHCHECK 无法同时覆盖两者。
COPY infrastructure/docker/healthcheck.sh /usr/local/bin/healthcheck.sh
RUN chmod +x /usr/local/bin/healthcheck.sh

# Note: no COPY of config.yaml. The default config is embedded in the binary
# via `include_str!("../config/config.default.yaml")` (see src/config.rs) and
# is written to CONFIG_PATH on first boot if the file is absent. Runtime
# overrides come from env vars (see services/notification-service/AGENTS.md).

EXPOSE 8083

ENV HEALTHCHECK_PORT=8083

HEALTHCHECK --interval=15s --timeout=3s --retries=5 --start-period=20s \
  CMD /usr/local/bin/healthcheck.sh

CMD ["notification_service"]
