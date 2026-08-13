---
name: coder-perf-profile
description: |
  Performance and memory profiling workflow for this Rust microservices monorepo.
  Use when the user says "perf", "profiling", "heaptrack", "flamegraph",
  "memory leak", "RSS baseline", "内存", "性能", "火焰图", "low memory".
---

# coder-perf-profile

> **low-memory 原则**：本仓库目标是极低 RSS，任何使 baseline RSS 增长超过 20% 的改动必须附书面理由。

## 何时用

用户提到 perf / profiling / 火焰图 / 内存泄漏 / RSS baseline / 慢请求 / sqlx 慢查询 / 连接池时调用。

## 前置

- `docker compose -f infrastructure/docker-compose.yml up -d`（Prometheus :9090 / Grafana :3001 / Jaeger :16686 / OTel :4317）
- `target/` 如果是 root-owned：始终 `CARGO_TARGET_DIR=/tmp/<svc>-target`
- Linux host（perf 采样需要）

## 性能观测层次

```
Level 1 Metrics  → Prometheus（请求速率 / p99 / 错误率）
Level 2 Traces   → Jaeger（慢请求根因 span）
Level 3 Profile  → 火焰图（CPU / 内存热路径）
```
先 Level 1/2，确认为代码问题后再采 Level 3。

---

## CPU 火焰图（cargo flamegraph）

### 安装

```bash
cargo install cargo-flamegraph
# perf_events 需要（host 上）：
perf list 2>/dev/null || sudo sysctl kernel.perf_event_paranoid=-1
```

### musl 限制

`rust:1.96-slim-musl` 无 perf。用 glibc target 在 host 上分析：

```bash
CARGO_TARGET_DIR=/tmp/iam-target \
  cargo build -p iam-service --target x86_64-unknown-linux-gnu --release
```

### 采集

```bash
# 方法 1：cargo flamegraph（host 上跑，需要 sudo）
sudo cargo flamegraph \
  --bin iam-service \
  --root \
  --duration 60 \
  -- target/x86_64-unknown-linux-gnu/release/iam-service serve
# 输出：flamegraph.svg

# 方法 2：perf record → FlameGraph
perf record -g -- ./target/x86_64-unknown-linux-gnu/release/iam-service serve &
PERF_PID=$!; sleep 60; kill $PERF_PID

git clone --depth 1 https://github.com/brendangregg/FlameGraph.git /tmp/FlameGraph
perf script | /tmp/FlameGraph/stackcollapse-perf.pl | /tmp/FlameGraph/flamegraph.pl > flamegraph.svg
```

### 解析

- 纵轴 = 调用栈深度；横轴 = 样本占比（最宽 = CPU 热点）
- `Ctrl+F` 搜索函数名；悬停显示样本数 + 占比
- 最宽的顶层框 = 热点根因

---

## 内存分配火焰图（heaptrack）

### 核心约束：heaptrack 不支持 musl

heaptrack 依赖 glibc `malloc` hook，`x86_64-unknown-linux-musl` 目标**无法使用**。必须用 glibc 编译：

```bash
CARGO_TARGET_DIR=/tmp/iam-target \
  cargo build -p iam-service --target x86_64-unknown-linux-gnu --release
```

### 安装

```bash
# Ubuntu/Debian
sudo apt install heaptrack heaptrack-print
# macOS
brew install heaptrack
```

### 采集 + 解析

```bash
# 采集
heaptrack ./target/x86_64-unknown-linux-gnu/release/iam-service serve &
sleep 30; kill -SIGINT $!

# top 100 内存消耗路径
heaptrack_print heaptrack.$(ls -t heaptrack.*.gz | head -1 | rev | cut -d. -f2 | rev).gz | head -100

# 生成火焰图
git clone --depth 1 https://github.com/brendangregg/FlameGraph.git /tmp/FlameGraph
heaptrack_print heaptrack.20260101-123456.gz \
  | stackcollapse-heaptrack.pl \
  | flamegraph.pl --color=mem > memory_flamegraph.svg
```

---

## RSS baseline 对比

RSS（进程实际占用物理内存）是本 repo 核心指标。

```bash
#!/bin/bash
# scripts/rss-baseline.sh
SVC=${1:-iam-service}
BINARY="target/x86_64-unknown-linux-gnu/release/${SVC}"
get_rss() { grep VmRSS /proc/$(pgrep -f "$BINARY.*serve" | head -1)/status | awk '{print $2}'; }

# idle 基线
./$BINARY serve & sleep 8
IDLE=$(get_rss); echo "Idle RSS: ${IDLE} kB"; kill %1 2>/dev/null

# 施压（需要 wrk）
wrk -t2 -c50 -d30s http://localhost:6002/health &
sleep 3; LOAD=$(get_rss); echo "Load RSS: ${LOAD} kB"
echo "Delta: $((LOAD - IDLE)) kB (growth: $(( (LOAD - IDLE) * 100 / IDLE ))%)"
kill %1 2>/dev/null
```

---

## OTel span 定位慢请求

已启用 OTel 的服务：`api-gateway` / `token-service` / `iam-service` / `notification-service`。

### Prometheus 找慢 endpoint

```promql
topk(10,
  sum by (path) (rate(http_request_duration_seconds_sum[5m]))
  / sum by (path) (rate(http_request_duration_seconds_count[5m]))
)
```

### Jaeger 找 trace

```bash
curl -si http://localhost:3000/v1/iam/health | grep X-Trace-Id
xdg-open "http://localhost:16686/trace/<trace_id>"
```

Service 选 `api-gateway`，Operation 选 `http_request`，按 Duration 降序，展开最慢 trace 的 span 树。

### 代码加自定义 span

```rust
use tracing::instrument;

#[instrument(skip(pool), fields(db.pool.size = pool.size()))]
async fn query_users(pool: &sqlx::MySqlPool) -> Result<Vec<User>> {
    // ...
}
```

---

## sqlx 慢查询 + 连接池监控

### 开启查询日志

连接字符串加 `log_statements`：

```rust
MySqlPoolOptions::new()
    .max_connections(10)
    .connect_with(
        "mysql://user:pass@localhost:3306/db"
            .parse::<MySqlConnectOptions>()?
            .log_statements(tracing::log::LevelFilter::Debug)
    )
    .await?;
```

### 手动暴露连接池指标

```rust
use prometheus::{IntGauge, register_int_gauge};
static DB_POOL_SIZE: IntGauge = register_int_gauge!("db_pool_size", "pool size").unwrap();
static DB_POOL_IDLE: IntGauge = register_int_gauge!("db_pool_idle", "idle conns").unwrap();

fn scrape(pool: &sqlx::MySqlPool) {
    DB_POOL_SIZE.set(pool.size() as i64);
    DB_POOL_IDLE.set(pool.num_idle() as i64);
}
```

### 连接池耗尽告警（PromQL）

```promql
# 连接数超过 80% 上限
db_pool_size{service="iam-service"} / 10 > 0.8
# 无空闲连接
db_pool_idle{service="iam-service"} == 0
```

---

## 性能回归防御 check-list

提交前自检。任意一项为"是"需要修复或附理由。

| 检查项 | 命令 | 危险信号 |
|--------|------|----------|
| 热路径 clone | `grep -rn "\.clone()" services/<svc>/src/ --include="*.rs" \| grep -v test` | 循环内或每次请求必走 |
| async hot path 中的 sleep | `grep -rn "tokio::time::sleep" services/<svc>/src/ --include="*.rs" \| grep -v test` | handler 内阻塞 |
| panic 路径的 unwrap | `grep -rn "\.unwrap()\|\.expect(" services/<svc>/src/ --include="*.rs" \| grep -v test` | I/O / 解析结果上 unwrap |
| 未限流 spawn | `grep -rn "for.*tokio::spawn" services/<svc>/src/ --include="*.rs" -A 2 \| grep -v test` | 无 Semaphore 包装 |

### 常见修复

**clone 替换**：`&[T]` / `Arc<[T]>` / `Cow<'_, str>`
**sleep 替换**：`tokio::time::timeout()` 或非阻塞 retry
**unwrap 替换**：`?` + `thiserror` / `.ok_or_else()`
**spawn 限流**：

```rust
let sem = Arc::new(Semaphore::new(10));
for item in items {
    let permit = sem.clone().acquire().await?;
    tokio::spawn(async move {
        process(item).await;
        drop(permit);
    });
}
```

---

## 验证

```bash
cargo check -p <svc> 2>&1 | grep -E "warning|error"
cargo test -p <svc> --lib
node scripts/check-service-standard.js --service <svc>
node scripts/dump-openapi-specs.js && git diff --exit-code services/<svc>/openapi.yaml
```
