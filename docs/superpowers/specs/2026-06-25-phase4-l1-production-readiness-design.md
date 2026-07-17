# Phase 4 L1 · Production Readiness Design

> 日期：2026-06-25
> 状态：草案 → 待用户复审
> 关联 roadmap：[2026-06-25-overall-roadmap.md](../plans/2026-06-25-overall-roadmap.md)
> 前置：12 周 roadmap（P1–P3）已交付；本期把"功能闭环"升级为"敢上 staging"

---

## 1. 背景与动机

12 周 roadmap 完成后，系统在 dev 环境完整闭环：8 个服务 + 可观测性栈 + 灰度 + 限流。
但**没有一个团队成员敢把它部署到 staging**，原因：

| 问题 | 现状 | 风险 |
|------|------|------|
| 没有优雅停机 | 4 个服务用 `std::process::exit(0)` 硬终止 | 滚动发布出 5xx |
| 无 readiness 分级 | `/health` 只返回 `"OK"`，不查依赖 | LB 在 DB 没连上时就把流量灌进来 |
| 无 CVE 闸门 | CI 不扫源码依赖或镜像 | 合规底线缺失 |
| 无 SLO 定义 | Prometheus 采数据但"什么算异常"无契约 | 值班人员不知道该响应什么 |
| OTel trace 未串通 | gateway 启用了，其它 3 个 service 没启 | 跨服务 trace 是 retro 列出但未做的事 |

**目标**：一条命令 `pnpm run check:readiness` 全绿，定义"production readiness L1"基线。

---

## 2. 范围

### In scope（本 spec）

1. **P4.1** Health 三级分离（`/health` 兼容、`/health/live`、`/health/ready`）+ 8 个服务统一优雅停机
2. **P4.2** CVE 闸门（`cargo audit` + `trivy image` 接 CI；high/critical 阻断；ignore 文件带 expiry）
3. **P4.3** 5 个 SLO + Prometheus alert rules + Alertmanager + mock-webhook 验证服务
4. **P4.4** 跨服务 trace e2e 断言（gateway → token → iam → audit，Jaeger query API）

### Out of scope（明确不做）

- K8s manifest / Helm chart（先 ADR-only，等运维上 K8s 决策再做）
- Secrets management（Vault / sops）
- 前端 e2e 覆盖率（放 Phase 5）
- 真实告警通道（企微/钉钉 webhook 把 mock-webhook 替换即可，无代码改动）
- 多区域 / 高可用上层（远期 mesh 升级）

---

## 3. 全局约束

- **不改对外 HTTP 契约**。新增端点只在 `/health/*` 路径下。`/health` 保留为旧调用方兼容（返回值不变）。
- **不动业务代码**。所有改动落在 `main.rs` 装配层 + 新 `libs/common-health` crate + 新 CI job + 新 e2e 用例。
- **可独立提交**。4 个子系统互不依赖，可任意顺序合入 main。
- **本地等价**。每个 CI job 必须有 `pnpm run check:*` 或脚本对应物，PM agent 能 dry run。
- **向后兼容**。`scripts/check-service-standard.js` 8-item baseline 不破坏旧规则，新规则以 "或" 方式追加（`/health` 或 `/health/live` 任一存在即通过；模板默认要 live+ready）。
- **网络约束**：访问 GitHub 需通过 `gh-proxy.org`；扫描器镜像走内网 `zhcoder-docker-registry.com:8000`。

---

## 4. 架构总览

```
┌─────────────────────────────────────────────────────────────────┐
│                    pnpm run check:readiness                     │
│                                                                 │
│  1. check:service-standard  (升级版，识别 /health/live, /ready)  │
│  2. check:cve               (cargo audit + trivy 本地等价)       │
│  3. check:cve-expiry        (ignore 文件过期警报)                │
│  4. promtool check rules    (alert rule 语法)                    │
│  5. test:readiness e2e      (health + alerts + trace 三合一)     │
└─────────────────────────────────────────────────────────────────┘
        ▲                  ▲                  ▲              ▲
        │                  │                  │              │
   P4.1 Health         P4.2 CVE           P4.3 SLO        P4.4 Trace
   ┌────────────┐      ┌──────────┐      ┌─────────┐     ┌─────────┐
   │ 8 services │      │ Cargo +  │      │ Prom    │     │ Jaeger  │
   │ refactored │      │ Trivy CI │      │ rules + │     │ query   │
   │ + common-  │      │ ignore   │      │ AM +    │     │ assert  │
   │   health   │      │ expiry   │      │ mock-wh │     │ 4 svc   │
   └────────────┘      └──────────┘      └─────────┘     └─────────┘
```

四个子系统互相不依赖，可并行 4 路 subagent 推进。每个子系统的产物（脚本 / 配置 / e2e 用例）在 § 5–§ 8 详述。

---

## 5. P4.1 Health 分级 + 优雅停机

### 5.1 端点契约

| 路径 | 用途 | 返回 | 检查 |
|------|------|------|------|
| `/health` | 兼容旧调用（路由 toml、Consul check） | `200 "OK"` | 进程活着 |
| `/health/live` | K8s liveness、ELB | `200 "OK"` | 进程活着，**零下游检查** |
| `/health/ready` | K8s readiness、LB 加流 | `200 {"db":"ok","redis":"ok",...}` 或 `503 {"db":"timeout"}` | DB ping + Redis ping，**并发**跑 |

**关键决策**（来自澄清问题）：readiness 只查 DB + Redis，**不查 Consul 注册状态**，避免启动慢。Consul 注册失败后台重试。

### 5.2 新 crate: `libs/common-health`

```rust
// libs/common-health/src/lib.rs
pub struct HealthState {
    pub db: Option<sqlx::AnyPool>,
    pub redis: Option<redis::aio::ConnectionManager>,
    pub shutting_down: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub ready_gauge: prometheus::IntGauge, // 暴露给 Prometheus 抓取，SLO #4 依据
}

impl HealthState {
    pub fn mark_shutting_down(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        self.ready_gauge.set(0);
    }
}

impl HealthState {
    pub fn new() -> Self { ... }
    pub fn with_db(mut self, pool: ...) -> Self { ... }
    pub fn with_redis(mut self, conn: ...) -> Self { ... }
}

pub fn routes(state: HealthState) -> axum::Router {
    Router::new()
        .route("/health", get(legacy))
        .route("/health/live", get(live))
        .route("/health/ready", get(|State(s)| async move { ready(s).await }))
        .with_state(state)
}

async fn legacy() -> &'static str { "OK" }
async fn live() -> &'static str { "OK" }
async fn ready(s: HealthState) -> impl IntoResponse {
    let mut report = serde_json::Map::new();
    let mut ok = true;
    if let Some(pool) = &s.db {
        match tokio::time::timeout(Duration::from_secs(1), sqlx::query("SELECT 1").execute(pool)).await {
            Ok(Ok(_)) => report.insert("db".into(), "ok".into()),
            _ => { report.insert("db".into(), "fail".into()); ok = false; }
        };
    }
    if let Some(conn) = &s.redis {
        match tokio::time::timeout(Duration::from_millis(500), conn.clone().ping::<()>()).await {
            Ok(Ok(_)) => report.insert("redis".into(), "ok".into()),
            _ => { report.insert("redis".into(), "fail".into()); ok = false; }
        };
    }
    let status = if ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    (status, Json(report))
}
```

**特例**：
- api-gateway 无 DB/Redis → readiness 检查 Consul TCP 连通（200ms 超时）。
- prue_backend SQLite in-memory → DB ping 默认通过。

### 5.3 优雅停机

替换 8 个服务的 main.rs 中的 `tokio::spawn { ctrl_c; exit(0) }` 模式为：

```rust
let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
let health_state = HealthState::new().with_db(pool.clone()).with_redis(redis.clone());

axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal(shutdown_tx.clone(), health_state, consul_registrar))
    .await
    .unwrap();

async fn shutdown_signal(
    tx: tokio::sync::watch::Sender<bool>,
    health: HealthState,
    mut consul: ConsulRegistrar,
) {
    let sigterm = async {
        signal(SignalKind::terminate()).unwrap().recv().await;
    };
    let sigint = tokio::signal::ctrl_c();
    tokio::select! {
        _ = sigterm => {},
        _ = sigint => {},
    }
    tracing::info!("shutdown signal received");

    // 1. 切 readiness false (LB 摘流)
    health.mark_shutting_down();
    let _ = tx.send(true);

    // 2. 等 LB 健康检查感知 (默认 5s, 通过 SHUTDOWN_DRAIN_SECS 可调)
    let drain = std::env::var("SHUTDOWN_DRAIN_SECS")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(5);
    tokio::time::sleep(Duration::from_secs(drain)).await;

    // 3. deregister consul
    let _ = consul.deregister().await;

    // axum graceful_shutdown 接管后续 in-flight drain
}
```

**HealthState 加 `shutting_down: AtomicBool`**：set 后 `/health/ready` 直接 503。

### 5.4 服务清单（8 个）

| Service | DB | Redis | 特殊 |
|---------|----|----|------|
| api-gateway | ✗ | ✗ | readiness 查 Consul TCP |
| token-service | ✓ | ✓ | 已有 ctrl_c handler，替换 |
| iam-service | ✓ | ✗ | 已有 ctrl_c handler，替换 |
| notification-service | ✓ | ✓ | 已有 ctrl_c handler，替换 |
| audit-service | ✓ | ✗ | 已有 ctrl_c handler，替换 |
| config-service | ✓ | ✗ | 当前无 ctrl_c，需新增完整 shutdown |
| file-service | ✓ | ✗ | 当前无 ctrl_c，需新增完整 shutdown |
| prue_backend | ✓ (SQLite ok) | ✗ | 当前无 ctrl_c，需新增完整 shutdown |

### 5.5 模板与脚本更新

- `services/agents-template.md` 必需组件 8 项扩到 9 项（追加"优雅停机 (SIGTERM + drain)"）。8 个 AGENTS.md 同步勾选。
- `scripts/check-service-standard.js` 新规则：
  - `/health` 端点 → 改为"`/health` 或 `/health/live` 任一存在"（向后兼容）
  - 新规则"`/health/ready` 端点"
  - 新规则"优雅停机（`with_graceful_shutdown` 或 `shutdown_signal` 出现在 main.rs）"

### 5.6 验收

- `pnpm run check:service-standard` exit 0
- 手动 `docker compose up -d && curl :3000/health/live` 返回 OK
- 手动 `curl :3000/health/ready` 在 Consul 起来前返回 503，起来后返回 200
- `docker kill --signal=SIGTERM <token-container>` 期间循环 curl `/token/issue` 零 5xx（容器在 ~5–10s 内停止）

---

## 6. P4.2 CVE 闸门

### 6.1 工具与时机

| 工具 | 对象 | 时机 | 配置 |
|------|------|------|------|
| `cargo audit` | `Cargo.lock` | CI test 阶段 + pre-commit hook（可选） | `.cargo/audit.toml` |
| `trivy image` | 8 个 docker 镜像 | CI build 阶段，push 镜像前 | `.trivyignore` |

### 6.2 严格度

- **critical + high** → CI exit 1，阻断 merge/deploy
- **medium + low** → 报告但 exit 0（持续可见度）
- 例外通过 ignore 文件显式声明，每条必须带：
  - `# reason: <说明>`
  - `# expires: YYYY-MM-DD`（脚本检查过期）
  - `# owner: <责任人/团队>`

### 6.3 CI job

```yaml
cargo_audit:
  stage: test
  tags: [docker]
  image: zhcoder-docker-registry.com:8000/official/rust:1.96-slim
  script:
    - cargo install --locked cargo-audit
    - cargo audit --color always --deny warnings
  allow_failure: false

trivy_scan:
  stage: build
  needs:
    - build_api_gateway
    - build_token_service
    - build_iam_service
    - build_notification_service
    - build_audit_service
    - build_config_service
    - build_file_service
    - build_prue_backend
  tags: [shell]
  parallel:
    matrix:
      - SERVICE: [api-gateway, token-service, iam-service, notification-service, audit-service, config-service, file-service, prue_backend]
  script:
    - trivy image --severity HIGH,CRITICAL --exit-code 1 \
        --ignorefile .trivyignore \
        $DOCKER_REGISTRY/$DOCKER_REPO/$SERVICE:$IMAGE_TAG

cve_expiry_check:
  stage: test
  tags: [docker]
  image: node:20
  script:
    - node scripts/check-cve-expiry.js
```

### 6.4 本地等价脚本

`scripts/check-cve.js`：
- 检测本机 `cargo-audit` 是否安装，否则跳过 cargo 部分并 warn
- 检测本机 `trivy` 是否安装，否则跳过镜像部分并 warn
- 运行后输出 high/critical 数量；任一非 0 → exit 1
- 在 CI 环境下，缺工具 = exit 1（避免静默通过）

`scripts/check-cve-expiry.js`：
- 解析 `.cargo/audit.toml`、`.trivyignore`
- 提取 `# expires: YYYY-MM-DD`；过期 → exit 1，列出条目
- 14 天内即将过期 → exit 0 + warn

### 6.5 验收

- `pnpm run check:cve` 在 clean 仓库 exit 0
- 故意引入一个 high CVE crate（用 `RUSTSEC-2020-0071` 测试 crate） → exit 1
- 在 ignore 加 expired 条目 → `check:cve-expiry` exit 1

---

## 7. P4.3 SLO + Alert + mock-webhook

### 7.1 5 个 SLO 定义

写入 `docs/slo.md`（新文件）：

| # | SLO 名 | 目标 | PromQL alert (草稿) | window | severity |
|---|--------|------|---------------------|--------|---------|
| 1 | gateway-5xx-rate | <0.5% (5min sum) | `sum(rate(http_requests_total{service="api-gateway",status=~"5.."}[5m])) / sum(rate(http_requests_total{service="api-gateway"}[5m])) > 0.005` | 15min | high |
| 2 | token-issue-p99 | <200ms | `histogram_quantile(0.99, rate(http_request_duration_seconds_bucket{service="token-service",route="/token/issue"}[5m])) > 0.2` | 10min | high |
| 3 | iam-login-p99 | <500ms | `histogram_quantile(0.99, rate(http_request_duration_seconds_bucket{service="iam-service",route="/iam/auth/login"}[5m])) > 0.5` | 10min | high |
| 4 | readiness-fail | 任一服务连续 fail >1min | `min_over_time(service_ready[1m]) == 0` | 1min | critical |
| 5 | e2e-smoke-cron | 连续失败 >2 轮 | `min_over_time(e2e_smoke_pass[2h]) == 0` | 2 轮 | critical |

### 7.2 文件落点

- `infrastructure/config/prometheus/alerts.yml` — 5 条 alert rule
- `infrastructure/config/prometheus/prometheus.yml` 加 `rule_files: [alerts.yml]`
- `infrastructure/config/alertmanager/alertmanager.yml`（新建）— 路由规则：
  ```yaml
  route:
    receiver: mock-webhook
    routes:
      - matchers: [severity="critical"]
        receiver: mock-webhook
        group_wait: 0s
      - matchers: [severity="high"]
        receiver: mock-webhook
        group_wait: 30s
  receivers:
    - name: mock-webhook
      webhook_configs:
        - url: http://mock-webhook:9000/webhook
  ```
- `infrastructure/docker-compose-observability.yml` 加 `alertmanager` 与 `pushgateway` 与 `mock-webhook` 三个 service

### 7.3 mock-webhook 服务

新 service: `services/mock-webhook/`

| 项 | 说明 |
|----|------|
| 端口 | 9000 |
| 路由 | `POST /webhook`（接 Alertmanager），`GET /webhook/received`（e2e 查询，返回最近 100 条 JSON 数组），`DELETE /webhook/received`（e2e 清空） |
| 存储 | 内存 RingBuffer（最近 100） |
| 大小 | <100 行 axum 代码，<10MB 镜像 |
| 用途 | dev + CI，**不打生产镜像** |

> 注：mock-webhook 是 dev/CI 工具，不上 production 服务规范（在 `check-service-standard.js` 中通过路径或 AGENTS.md 豁免标记排除）。

### 7.4 e2e-smoke-cron + Pushgateway

`scripts/e2e-smoke-cron.sh`（新）：

```bash
#!/bin/bash
set -e
cd tests/e2e
if pnpm run test:smoke; then
  RESULT=1
else
  RESULT=0
fi
cat <<EOF | curl --data-binary @- http://pushgateway:9091/metrics/job/e2e_smoke
# TYPE e2e_smoke_pass gauge
e2e_smoke_pass $RESULT
EOF
```

Cron 调度交给运维（README 说明）；CI 不阻断在 cron，但 SLO #5 通过 Prometheus 检测 `e2e_smoke_pass`。

### 7.5 e2e 验证用例 `tests/e2e/alerts.spec.ts`

```ts
test('SLO #4 readiness-fail triggers alert in <90s', async () => {
  // 1. 清空 mock-webhook
  await fetch(`${mockWebhook}/webhook/received`, { method: 'DELETE' })
  // 2. 触发 token-service 临时不 ready (注入 SHUTDOWN_DRAIN_SECS=300, kill -SIGTERM)
  await injectReadinessFail('token-service')
  // 3. 轮询 mock-webhook，断言 60-120s 内收到 alertname=readiness-fail
  await pollUntil(async () => {
    const alerts = await fetch(`${mockWebhook}/webhook/received`).then(r => r.json())
    return alerts.some(a => a.alerts.some(x => x.labels.alertname === 'readiness-fail'))
  }, { timeout: 120000 })
})
```

### 7.6 验收

- `promtool check rules infrastructure/config/prometheus/alerts.yml` exit 0
- `docker compose -f docker-compose.yml -f docker-compose-observability.yml up -d` 后能在 `http://localhost:9093` 看到 alertmanager UI
- `cd tests/e2e && pnpm run test:alerts` exit 0
- 故意触发 readiness-fail，mock-webhook 在 90s 内收到 webhook

---

## 8. P4.4 跨服务 trace 验证

### 8.1 前置：token / iam / audit 启用 OTel

`docs/superpowers/retros/2026-06-retrospective.md` § 7 列为下月目标。本期落实：

- 复制 api-gateway 的 OTel 初始化代码到 `libs/common-otel`（新 crate 或扩 common-log）
- 4 个 service（token / iam / notification / audit）的 main.rs 调用 `common_otel::init("service-name")`
- 各 service `Cargo.toml` 加 `opentelemetry`, `opentelemetry-otlp`, `tracing-opentelemetry`（与 gateway 同版本，已知 0.17 + `WithExportConfig`）
- 加 axum middleware 提取 `traceparent` header 并 set parent span（用 `tracing-opentelemetry::OpenTelemetrySpanExt::set_parent`）

### 8.2 trace_id 响应头

所有 service 加同一个 axum response middleware，把当前 span 的 trace_id 写入 `x-trace-id` 响应头，方便 e2e 抓取。

### 8.3 e2e 用例 `tests/e2e/trace.spec.ts`

```ts
test('gateway → token → iam trace propagation', async () => {
  const res = await fetch(`${gw}/iam/auth/login`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ username: 'admin', password: 'admin' }),
  })
  const traceId = res.headers.get('x-trace-id')
  expect(traceId).toMatch(/^[0-9a-f]{32}$/)

  await pollUntil(async () => {
    const r = await fetch(`${jaegerQuery}/api/traces/${traceId}`)
    if (!r.ok) return false
    const trace = await r.json()
    if (!trace.data?.[0]) return false
    const services = new Set(trace.data[0].spans.map(s => s.process.serviceName))
    return services.has('api-gateway')
        && services.has('token-service')
        && services.has('iam-service')
  }, { timeout: 30000, interval: 1000 })
})

test('audit log path trace propagation', async () => {
  // 触发会写 audit log 的操作
  // 断言同 traceId 在 audit-service 也有 span
})
```

### 8.4 CI job

```yaml
e2e_trace:
  stage: test
  tags: [docker]
  image: mcr.microsoft.com/playwright:v1.48.0-jammy
  before_script:
    - docker compose -f infrastructure/docker-compose.yml -f infrastructure/docker-compose-observability.yml up -d
    - sleep 30  # 等所有 service ready
  script:
    - cd tests/e2e && pnpm run test:trace
  artifacts:
    paths:
      - tests/e2e/test-results/trace/
```

### 8.5 验收

- `cd tests/e2e && pnpm run test:trace` exit 0
- Jaeger UI (`http://localhost:16686`) 能看到至少一条 4-service trace（gateway → token → iam → audit）

---

## 9. 一致性闸门：`pnpm run check:readiness`

`package.json` 新增：

```json
"scripts": {
  "check:readiness": "pnpm run check:service-standard && pnpm run check:cve && pnpm run check:cve-expiry && pnpm run check:alerts-syntax && cd tests/e2e && pnpm run test:readiness",
  "check:cve": "node scripts/check-cve.js",
  "check:cve-expiry": "node scripts/check-cve-expiry.js",
  "check:alerts-syntax": "docker run --rm -v $(pwd)/infrastructure/config/prometheus:/etc/prom prom/prometheus:latest promtool check rules /etc/prom/alerts.yml"
}
```

`tests/e2e/package.json` 新增：

```json
"scripts": {
  "test:readiness": "playwright test health.spec.ts alerts.spec.ts trace.spec.ts"
}
```

---

## 10. ADR-003 草稿

新文件：`docs-site/decisions/ADR-003-production-readiness-baseline.md`

固化：
- 4 项 L1 基线（health 分级 + 优雅停机 / CVE 闸门 / 5 SLO + alert / 跨服务 trace）
- 严格度（high/critical 阻断、low/medium 警告）
- 5 个 SLO 名称与目标值（PromQL 阈值在 alerts.yml，可调）
- `pnpm run check:readiness` 作为 L1 边界

---

## 11. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| `common-health` 引入循环依赖 | 编译失败 | crate 只依赖 sqlx + redis crate（已在 workspace），不依赖任何 service crate |
| OTel 4-service 启用引入 0.17 API 兼容坑 | trace 不上报 | retro 已记录 `WithExportConfig` + `trace` feature；本期把它写进 `common-otel` 一次解决 |
| mock-webhook 被误打成 production 镜像 | 体积膨胀 | dockerfile / 服务规范脚本通过路径/AGENTS 标记排除；ADR 显式声明 |
| Alertmanager 内存 SLO #4 触发依赖 readiness 探针被 Prometheus 抓取 | SLO #4 不触发 | `common-health` 暴露 `service_ready` IntGauge 到 `/metrics`，Prometheus 已有 SD 自动抓所有服务的 metrics endpoint，无需新增 job |
| Trivy 在内网无法访问 vuln DB | CI fail | 在 CI runner 提前 `trivy image --download-db-only`；或使用离线 DB cache |
| `pnpm run check:readiness` 总耗时过长 | DX 退化 | 4 段并行设计；目标 <3min（unit 测试已习惯 2min） |

---

## 12. 验收清单

- [ ] 8 个服务 `/health/live` + `/health/ready` 全部上线，readiness 包含 DB + Redis 检查
- [ ] 8 个服务 main.rs 用 `axum::serve(...).with_graceful_shutdown(...)`，无 `std::process::exit(0)`
- [ ] `services/agents-template.md` 与 8 个 AGENTS.md 升级到 9-item baseline
- [ ] `scripts/check-service-standard.js` 9 项规则 exit 0
- [ ] `.cargo/audit.toml` + `.trivyignore` 在仓库；条目带 `# reason / # expires / # owner`
- [ ] `cargo_audit` / `trivy_scan` / `cve_expiry_check` 三个 CI job 接入并绿
- [ ] `scripts/check-cve.js` 本地能 exit 0
- [ ] `infrastructure/config/prometheus/alerts.yml` 5 条 rule，`promtool check rules` exit 0
- [ ] `infrastructure/config/alertmanager/alertmanager.yml` 路由到 mock-webhook
- [ ] `services/mock-webhook/` 上线，docker-compose-observability 起来
- [ ] `tests/e2e/alerts.spec.ts` 能断言 SLO #4 触发
- [ ] 4 个服务（token/iam/notification/audit）启用 OTel，`x-trace-id` 响应头存在
- [ ] `tests/e2e/trace.spec.ts` 能断言 4-service trace 联贯
- [ ] `docs-site/decisions/ADR-003-production-readiness-baseline.md` 提交
- [ ] `docs/slo.md` 提交
- [ ] `pnpm run check:readiness` 一条命令全绿
- [ ] roadmap.md 增加 Phase 4 L1 段落与指标表行

---

## 13. 关联入口

- 本 spec：`docs/superpowers/specs/2026-06-25-phase4-l1-production-readiness-design.md`
- 实施 plan（待 writing-plans 产出）：`docs/superpowers/plans/2026-06-25-phase4-l1-plan.md`
- 总 roadmap：`docs/superpowers/plans/2026-06-25-overall-roadmap.md`
- 上个月 retro：`docs/superpowers/retros/2026-06-retrospective.md`
- 服务规范基线 ADR-002：`docs-site/decisions/ADR-002-service-standard-baseline.md`
- L1 baseline ADR-003：`docs-site/decisions/ADR-003-production-readiness-baseline.md`（待创建）
