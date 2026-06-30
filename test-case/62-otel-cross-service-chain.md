# T2: OTel 全覆盖 + tracing_mw 提取 — 端到端 trace 链验收

## 前置

```bash
cd /home/kudi/projects/templates/low-memory-micro-services
docker compose -f infrastructure/docker-compose-infra.yml up -d
# 启动 8 个服务：api-gateway / iam-service / token-service / notification-service /
#                audit-service / config-service / file-service / prue_backend
# 注意：4 个新接入服务（audit/config/file/prue_backend）必须按 T2 后的 Cargo.toml 编译
```

## 步骤

### 1. 各服务 OTel init 验证

启动 audit / config / file / prue_backend 各服务，确认日志首行有
`OTel tracing enabled → http://otel-collector:4317`。

如未设 `OTEL_EXPORTER_OTLP_ENDPOINT`，应回退到 stdout JSON log
（`tracing initialized (stdout only, no OTLP)`）。

### 2. 跨服务 trace 链验证

起 iam-service + outbox-publisher + audit-service（这 3 个有事件 producer/consumer 链）。

执行：
```bash
TRACE_ID="aabbccddeeff00112233445566778899"
curl -X POST http://localhost:3000/v1/iam/users \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "traceparent: 00-${TRACE_ID}-1122334455667788-01" \
  -d '{"username":"otel_test","password":"hunter2"}'
```

等 5s（outbox poll + audit consume）。

打开 Jaeger UI http://localhost:16686：
- Service: iam-service
- 搜 `trace_id == aabbccddeeff00112233445566778899`
- 应看到：
  1. api-gateway HTTP root
  2. iam-service HTTP child
  3. audit-service consumer span（set parent from envelope.trace_id）
  4. 三者同一 trace_id，组成完整 chain

### 3. traceparent 透传验证

对 audit / config / file / prue_backend 各自 `/health`：

```bash
curl -H "traceparent: 00-${TRACE_ID}-1122334455667788-01" \
     -i http://localhost:6005/health
```

响应 header `X-Trace-Id` 应为 `aabbccddeeff00112233445566778899`。

### 4. feature 矩阵验证（CI 已自动跑，本节为本地 sanity）

> 自 T2 fix 起，axum 中间件拆出到 `common-telemetry-axum` crate（plan §3 row 5
> 要求 outbox-publisher 不带 axum），两个 crate 各自只有 `default` / `otlp` 两态。

```bash
cargo check -p common-telemetry --no-default-features                 # OK
cargo check -p common-telemetry --features otlp                       # OK
cargo check -p common-telemetry-axum --no-default-features            # OK
cargo check -p common-telemetry-axum --features otlp                  # OK
```

附带 CLI binary 验证（关键：outbox-publisher 不应再带 axum）：

```bash
cargo tree -p iam-service -e features 2>&1 | grep -c "^.*axum "  # > 0
cargo tree -p iam-service -e features 2>&1 \
  | grep -A1 "outbox-publisher" | grep -c axum  # 期望 0
```

## 验收标准

- [ ] 4 个新接入服务（audit/config/file/prue_backend）首行日志显示 OTel init 成功或 stdout fallback
- [ ] iam → audit 跨服务 trace 链在 Jaeger 端到端可见
- [ ] audit / config / file / prue_backend `/health` 都透传 traceparent → X-Trace-Id 一致
- [ ] 2×2 feature 矩阵 cargo check 全通过（`common-telemetry` × `common-telemetry-axum` 各 2 态）
- [ ] outbox-publisher / ntf-outbox-publisher 的 `cargo tree` 不再带 axum
