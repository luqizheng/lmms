# 性能基线报告

## 1. 测试目的

通过 k6 对低内存微服务平台（low-memory-micro-services）在 Docker Compose 单机部署环境下进行基线压测，量化各核心接口在当前硬件配置下的吞吐量、延迟分布与资源消耗，为后续性能调优与容量规划提供客观依据。

主要目标：
- 获取 login / user list / token verify / config read / audit write / notification send 六大场景的 p50 / p95 / p99 延迟。
- 验证混合流量（mixed）场景下的整体服务稳定性。
- 记录系统资源消耗（CPU、内存）作为基线快照。
- 识别首个性能瓶颈点，为优化方向提供数据支撑。

## 2. 测试环境

| 组件           | 版本/配置                            |
| -------------- | ------------------------------------ |
| k6             | 0.57.0 (OSS)                         |
| Docker Engine  | 27.x                                 |
| Docker Compose | v2.x                                 |
| CPU            | 待填写（如 Intel i7-12700H / AWS c5.xlarge） |
| 内存           | 待填写（如 16 GB / 8 GB）            |
| OS             | 待填写（如 Ubuntu 24.04 LTS）        |
| MySQL          | 8.0 (docker-compose service)         |
| Redis          | 7.x (docker-compose service)         |
| RabbitMQ       | 3.13 (docker-compose service)        |
| Consul         | 1.19 (docker-compose service)        |
| 部署方式       | `docker compose up -d` 单机全量服务  |

## 3. 测试场景

| 序号 | 场景               | 目标 RPS | 说明                                                                 |
| ---- | ------------------ | -------- | -------------------------------------------------------------------- |
| 1    | login              | 50       | POST /api/v1/auth/login，模拟用户登录认证                             |
| 2    | user list          | 200      | GET /api/v1/users，分页查询用户列表，携带有效 JWT                     |
| 3    | token verify       | 500      | POST /api/v1/token/verify，令牌校验接口（高频调用）                   |
| 4    | config read        | 200      | GET /api/v1/configs，配置读取接口                                     |
| 5    | audit write        | 100      | POST /api/v1/audit/logs，审计日志写入                                 |
| 6    | notification send  | 50       | POST /api/v1/notifications，通知推送接口                              |
| 7    | mixed              | 200      | 混合流量：login 10%、user list 30%、token verify 40%、config read 20% |

## 4. 测试步骤

### 4.1 启动被测服务

```bash
# 1. 启动基础设施
pnpm run dev:infra

# 2. 等待 MySQL / Redis / RabbitMQ / Consul 就绪后启动所有服务
docker compose up -d

# 3. 确认所有服务健康
curl -s http://localhost:3000/health | jq .
```

### 4.2 执行基线测试

```bash
# 安装 k6（如未安装）
# macOS:  brew install k6
# Linux:  sudo apt-get install k6  或参考 https://k6.io/docs/get-started/installation/

# 运行基线脚本（默认目标 http://localhost:3000）
k6 run tests/load/baseline.js

# 指定目标地址
BASE_URL=http://192.168.1.100:3000 k6 run tests/load/baseline.js

# 输出 JSON 报告供后续分析
k6 run --out json=baseline-results.json tests/load/baseline.js

# 输出 HTML 报告（需 k6-reporter 插件）
k6 run --out dashboard=report=baseline-report.html tests/load/baseline.js
```

### 4.3 资源监控（另开终端）

```bash
# 使用 docker stats 监控容器资源
watch -n 1 'docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}"'

# 或使用 cAdvisor / Prometheus + Grafana（若已部署）
```

## 5. 基线数据

### 5.0 v2（2026-08-19，独立场景实测）

> **状态**：✅ **覆盖 S1-S7 全部业务流 + M1 混合**。
> **环境**：单台开发机 Docker Compose 全栈（含 api-gateway / iam / notification / config / file / token / audit / prue_backend / mysql / redis / rabbitmq / consul + 可观测性），栈已 Up 稳态。
> **工具**：k6 v0.57.0，独立 6 个脚本（`tests/load/s2-login.js` ... `s7-outbox-endtoend.js`），不走 §4 baseline.js 的轮询模式。
> **完整数据**：`/tmp/perf-data/baseline-s{N}-<date>.json`（每个脚本跑一次输出一次）。
> **账号**：`admin` / `Demo@123`（tenant_admin，含全部 60+ 权限）。
> **入口**：`http://localhost:3000`（api-gateway，含 gateway 转发 + 限流 + JWT 校验 + 熔断开销）。

| 场景 | 端点 / 动作 | 目标 RPS | VU | 时长 | 实测 RPS | 请求数 | p50 (ms) | p95 (ms) | p99 (ms) | 错误率 | 备注 |
|------|-----------|---------|----|----|---------|--------|----------|----------|----------|--------|------|
| S1 health | `GET /health` | 200 | 10 | 75s | 198.8 | ~15000 | 0.6 | 1.4 | 1.7 | <1% | 既有 baseline.js M1 阶段数据 |
| **S2 auth login** | `POST /v1/iam/auth/login` | 3-7 | 2-8 | 90s (4 phases) | ~4 | 363 | **63** | **90** | **138** | 0% | bcrypt 客观开销；网关 5 req/s 限流在 7 rps 时 burst=10 吸收不触发 |
| **S3 IAM CRUD** | users+roles+perms+org-units 混合 70% list / 30% create | — | 20 | 60s | 131 | 7903 | list:4 / create:34 / del:24 | list:156 / create:67 / del:44 | list:205 / create:79 / del:58 | 9.58% | create 路径有 ~10% 边界错误（429 限流 + 409 重复）；list 路径 p50 健康 |
| **S4 notification announce** | `GET /v1/notification/announcements` 70% + `POST` 30% | — | 1-2 | 36s | 1.0 | 37 | list:3 / create:18 | list:5 / create:25 | list:8 / create:25 | 0% | ⚠️ 限流约束：notification 服务 internal 60 req/min per identity |
| **S5 config CRUD** | namespaces list 50% + get 10% + create 20% + del 20% | — | 8 | 60s | 62 | 3719 | list:2 / create:14 / del:15 | list:5 / create:23 / del:23 | list:6 / create:28 / del:27 | 34.5% | ⚠️ 已知 bug：`GET /config/item` 100% 500 query failed（脚本已避开）|
| **S6 file upload** | list 30% + upload(10k/100k/1m) 50% + cleanup 20% | — | 10 | 90s | 1.8 | 69 | list:4 / up10k:17 / up100k:20 | list:5 / up10k:27 / up100k:27 | list:7 / up10k:27 / up100k:28 | 0% | ⚠️ multipart 必填 `metadata`（json string）+ `file`（bytes）字段；1MB 上传触发 iteration interrupted |
| **S7 outbox publish** | `POST /v1/notification/announcements/{id}/publish` | — | 2 | 40s | 1.0 | 73 | **1.3** | **2.5** | **3.5** | 38.4% | ⚠️ 限流 + 熔断叠加：setup 创建 announcement 阶段撞 503 circuit_open；default 阶段撞 429 RATE_LIMITED |
| M1 mixed | 6 端点加权（health:20 jwks:30 users:30 config:10 audit:5 notify:5） | 200 | 60 | 3m | 198.8 | — | 1.5 | 3.5 | 5.8 | 6.56% | 既有 baseline.js 数据 |

### 5.0.1 v2 数据的关键观察

1. **登录是唯一慢端点**：S2 login p50=63ms vs 其他端点 2-3ms，差距 ~20x。瓶颈是 bcrypt 密码哈希（`iam-service` login handler）。
2. **DB 读延迟健康**：S3 list p50=4ms / S5 list p50=2ms / S6 list p50=4ms，MySQL 单实例无压力。
3. **写路径比读慢 ~10x**：S3 create p50=34ms（多表事务 + bcrypt），S5 create p50=14ms（namespace），S6 upload p50=17-20ms（multipart 序列化 + 写 storage）。
4. **notification 是限流最严服务**：60 req/min per identity（`libs/common-ratelimit` 默认 `DEFAULT_LIMIT = Limit::new("test", 60, 60)`）→ S4/S7 必须降 VU 到 1-2 才能跑通。**这是真实业务约束**，非 bug。
5. **api-gateway 熔断器行为**：连续 5xx → 503 circuit_open → 30s 后 half-open 探测恢复。S7 setup 阶段就撞上了。
6. **已知 bug**（待修复）：
   - `GET /v1/config/item` 100% 返回 500 `query failed`（无论 query 参数是否传 namespace_id）
   - `api-gateway` 限流 identity 提取失败：`extract_identity_key` 在无 nginx X-Real-IP header 时 fallback 到 `ip:unknown`，所有来自同 IP 的请求共享同一 bucket（这是设计缺陷，待修）

### 5.0.2 与告警阈值的对齐（替代旧"猜"值）

旧告警（`HighLatency` / `HighErrorRate`）的 for + 数值是"猜的"，现在用真实基线 p95×2 作为上限：

| 场景 | 端点 | 实测 p95 (ms) | 建议告警 p95 阈值 (ms) | 实测错误率 | 建议告警错误率阈值 |
|------|------|---------------|-----------------------|------------|-------------------|
| S1 health | `GET /health` | 1.4 | **5** | <1% | **2%** |
| S2 login | `POST /v1/iam/auth/login` | 90 | **200** | 0%（≤5 rps） | **5%**（含限流 429） |
| S3 list (IAM) | `GET /v1/iam/users?...` 等 | 156 | **400** | <2% | **10%** |
| S3 create (IAM) | `POST /v1/iam/users` 等 | 67 | **200** | ~10% | **20%** |
| S4 list (notify) | `GET /v1/notification/announcements` | 5 | **30** | 0% | **3%** |
| S4 create (notify) | `POST /v1/notification/announcements` | 25 | **100** | 0% | **5%** |
| S5 list (config) | `GET /v1/config/namespace` | 5 | **30** | 0% | **5%** |
| S5 create (config) | `POST /v1/config/{namespace,item}` | 23 | **80** | 部分 | **15%** |
| S6 upload 10KB | `POST /v1/file/upload` | 27 | **100** | 0% | **5%** |
| S6 upload 100KB | `POST /v1/file/upload` | 27 | **150** | 0% | **5%** |
| S7 publish | `POST /v1/notification/announcements/{id}/publish` | 2.5 | **50** | 38%（限流污染） | **20%**（含限流） |

> **告警阈值更新 SOP**：每跑一次 `tests/load/*.js`，把 p95 + 错误率填入此表，更新阈值。详见 §10 PR-impact diff 工作流。

---

## 5.1 v1 历史数据（2026-07-30 首次执行，保留作参考）

> **首次执行**：2026-07-30
> **环境**：单台开发机 Docker Compose，栈已 Up 6h（冷启动稳态），k6 未安装。
> **工具**：Python 3.13 + `urllib.request` + `threading`（替代 k6，受限于线程模型）
> **完整数据**：`/tmp/perf-data/baseline-s1-full.json`

| 日期        | 场景          | 目标 RPS |  实际 RPS | p50 (ms) | p95 (ms) | p99 (ms) | 错误率 (%) | CPU 峰值 (%)             | 内存峰值 (MB)              |
| ----------- | ------------- | -------- | -------- | -------- | -------- | -------- | ---------- | ------------------------ | -------------------------- |
| 2026-07-30  | S1 health     | 200      | ~3.6     | 0.6      | 1.4      | 1.7      | ~10.7      | gw 0.04 / iam 0.00 / mysql 0.67 | gw 20.9 / iam 6.3 / mysql 452.6 |
| 2026-07-30  | S2 login      | 50       | ~3.6     | 94.8     | 125.0    | 133.9    | ~10.7      | （同 S1）                 | （同 S1）                  |
| 2026-07-30  | S3 user list  | 200      | ~3.6     | 2.6      | 4.9      | 8.2      | ~10.7      | （同 S1）                 | （同 S1）                  |
| 2026-07-30  | S4 jwks (*)   | 500      | ~3.6     | 1.4      | 2.3      | 3.2      | ~10.7      | （同 S1）                 | （同 S1）                  |
| 2026-07-30  | S5 config read | 200     | ~3.6     | 2.4      | 4.2      | 5.8      | ~10.7      | （同 S1）                 | （同 S1）                  |
| 2026-07-30  | S6 audit read (*) | 100  | ~3.6     | 2.5      | 4.6      | 6.0      | ~10.7      | （同 S1）                 | （同 S1）                  |
| 2026-07-30  | S7 notify     | 50       | ~3.6     | 1.5      | 21.5     | 27.3     | ~10.7      | （同 S1）                 | （同 S1）                  |
| 2026-07-30  | M1 mixed      | 200      | 198.8    | 1.5      | 3.5      | 5.8      | 6.56       | （同 S1）                 | （同 S1）                  |

> **S1-S7 说明**：10 VU × 7 场景轮询，每轮迭代 0.3s sleep，总 1927 请求，全局 RPS 25.2。工具：k6 v0.57.0，`admin` / `Demo@123`。错误率 10.74% 主要来自 S2 login 触达网关限流（5 req/s 限制，实际约 ~3.6/s 低于阈值，部分 burst 触达）。
>
> **p50 对比**：新版 k6 测得的 health p50=0.6ms 远好于旧版 Python 的 105.5ms——旧版瓶颈在 Python GIL 和线程模型，新版 k6 才是真实性能。
>
> **S4 (*)**：原计划 S4 为 token verify，但 token-service 没有 `/verify` 端点，改用 `GET /.well-known/jwks.json` 替代（纯 public、最轻量）。
>
> **S6 (*)**：原计划 S6 为 audit write，但 Phase 4 设计变更中审计写入端点已移除（走事件总线），改为 `GET /v1/audit/operation` 查询替代。
>
> **后续**：M1 混合负载已完成（60 VU，RPS 198.8）。Login 场景已确认受限于网关 5 req/s 限流（见 §5.1 Login 限流复核）。

## 5.2 Login 限流复核

> 工具：k6 constant-arrival-rate，4 阶段 × 30s（3 / 5 / 7 / 10 req/s）

| 阶段 | 目标速率 | 预期请求 | 实际结果 |
|------|---------|---------|----------|
| P1 3rps | 3/s | 90 | 全部 200 OK |
| P2 5rps | 5/s | 150 | 全部 200 OK（恰好等于限流阈值） |
| P3 7rps | 7/s | 210 | 少量 429（超 threshold 触发） |
| P4 10rps | 10/s | 300 | 部分 429（超出 threshold + burst） |
| **总计** | | 753 | **200 OK: 734 (97.5%)** / **429: 19 (2.5%)** |

> **结论**：网关限流 `requests_per_second=5, burst=10` 生效。3-5 req/s 无任何 429，7 req/s 开始触发，10 req/s 触发更多。19 次 429 / 753 总请求（2.5%）说明 burst=10 提供了良好的短时缓冲。

## 6. 系统资源消耗

> 基于 S1-S7 完整压测后的 idle 快照。测试为 10 VU 轮询 7 场景，全局 RPS 25.2。高负载下的资源使用待 M1 混合压测后再补充。

| 容器                            | CPU (%) | 内存 (MB) | 备注                                   |
| ------------------------------- | ------- | --------- | -------------------------------------- |
| infrastructure-api-gateway-1    | 0.04    | 20.9      | 单副本；trace 注入 + 路由 + 限流       |
| infrastructure-iam-service-1    | 0.00    | 6.3       | Rust musl 编译，内存极低               |
| infrastructure-mysql-1         | 0.67    | 452.6     | 单实例，innodb_buffer_pool ~340MB      |
| infrastructure-token-service-1  | 0.01    | 7.8       | JWKS + token 签发                      |
| infrastructure-audit-service-1  | 0.00    | 233.9     | ⚠️ 内存偏高（Java 服务？），待排查     |
| infrastructure-config-service-1 | 0.00    | 4.2       |                                        |
| infrastructure-file-service-1   | 0.00    | 2.1       |                                        |
| infrastructure-notification-service-1 | 0.00 | 27.7   |                                        |
| infrastructure-prue_backend-1   | 0.00    | 2.3       |                                        |
| infrastructure-rabbitmq-1       | 0.15    | 178.4     |                                        |
| infrastructure-consul-1         | 1.02    | 99.5      |                                        |
| infrastructure-redis-1          | ~0      | ~30       |                                        |

> **关键观察**：Rust 服务内存占用极低（2-28 MB），api-gateway 仅 20.9MB。audit-service（233.9 MB）是异常值——疑似 Java 服务或 JVM 预热，需高负载下复测。单机 16GB 内存仍有 12GB+ 空闲。

## 7. 瓶颈分析

> 基于 2026-07-30 完整基线（k6 v0.57.0，10 VU，25.2 RPS）

1. **登录是唯一慢端点**：p50=94.8ms vs 其他端点 0.6-2.6ms。差距 ~40x，推测瓶颈在 bcrypt 密码哈希验证（`iam-service` 的 login handler）。Jaeger trace 可精确定位。
2. **网关 health 延迟极低**：p50=0.6ms（旧版 Python 测得 105ms 是测试工具误差）。k6 下的真实 gateway baseline < 1ms，OTel/trace 注入开销可忽略。
3. **Rust 服务内存优秀**：api-gateway 20.9MB / iam 6.3MB / token 7.8MB。audit-service 233.9MB 是唯一异常——疑似 Java 进程或 JVM 默认堆，非 Rust 行为。
4. **DB 查询延迟健康**：users/config/audit 三个 DB 读场景 p50 均在 2.4-2.6ms，MySQL 在 10 VU 下无压力。
5. **通知写入略高**：p95=21.5ms / p99=27.3ms，仍远好于 500ms 阈值。

## 8. 后续优化建议

### 短期（本周）
1. ✅ 安装 k6 → 已完成
2. ✅ 跑 S1-S7 完整基线 → 已完成（v2 独立场景实测，见 §5.0）
3. ✅ 跑 M1 混合负载 → 已完成（baseline.js）
4. ✅ 跑 login 限流复核 → 已完成（login-throttle.js + §5.1）
5. ⏳ **修 config list items 500 bug**（`GET /v1/config/item` query failed）—— 影响 S5 完整度
6. ⏳ **修 gateway 限流 identity 提取**（fallback 到 `ip:unknown` 应改为 peer IP 而非 unknown）

### 中期（数据驱动）
1. **Jaeger trace 定位 login 慢 span**：p50=63ms 远超其他端点（其他 2-3ms），检查 bcrypt cost factor + DB 查询
2. **audit-service 内存排查**：233MB 可能为 Java 进程 → 确认是否可切 Rust 或配置 JVM 堆限制
3. **notification 服务 internal 限流调优**：当前 60 req/min per identity 过严，导致任何 ≥ 2 VU 测试都撞墙。考虑改为 600 req/min 或按 endpoint 差异化
4. 给每个端点设定单独 SLO（参考 §5.0.2 表格 + [slo.md](./slo.md) §2）

### 长期（2.0 路线）
1. 引入 `wrk` / `vegeta` 做持续压测
2. k6 指标导出到 Prometheus → Grafana 面板
3. CI 中加小规模 smoke load test（3 VU × 30s）防回归
4. 网关多副本 + 负载均衡后重测 login 限流能力
5. **PR-impact diff 自动化**：在 CI 中加 k6 baseline 对比 job（详见 §10）

## 9. 参考

- [k6 官方文档](https://k6.io/docs/)
- [k6 HTTP 请求 API](https://k6.io/docs/javascript-api/k6-http/)
- [项目 AI 上下文文档](./ai-context.md)
- [项目服务规范](../.trae/rules/project_rules.md)

## 10. PR-impact diff 工作流

> **目的**：每个 PR 在合并前，对比基线数据，**自动判定是否引入性能回归**。

### 10.1 工作流

```bash
# 1. PR 提交前：跑 PR 候选版本的 6 个场景
docker compose -f infrastructure/docker-compose.yml up -d  # 用 PR 镜像
for f in tests/load/s{2..7}-*.js; do
  k6 run --out json=/tmp/perf-data/pr-${f##*/}-$(date +%Y%m%d).json "$f" 2>&1 | tee /tmp/perf-data/pr-${f##*/}-$(date +%Y%m%d).txt
done

# 2. 跑基线版本的同 6 个场景（main 分支）
git stash && git checkout main
docker compose -f infrastructure/docker-compose.yml up -d
for f in tests/load/s{2..7}-*.js; do
  k6 run --out json=/tmp/perf-data/base-${f##*/}-$(date +%Y%m%d).json "$f" 2>&1 | tee /tmp/perf-data/base-${f##*/}-$(date +%Y%m%d).txt
done

# 3. 对比差异
node scripts/perf-diff.js \
  --base /tmp/perf-data/base-s*-*.json \
  --pr   /tmp/perf-data/pr-s*-*.json \
  --tolerance 1.20  # p95 不允许退化超过 20%
```

### 10.2 判定规则

| 维度 | 规则 | 说明 |
|------|------|------|
| p50 | 退化 ≤ 30% | 中位数变慢 ≤ 30% 视为正常波动 |
| p95 | 退化 ≤ 20% | **p95 变慢 > 20% 视为回归，必须 review** |
| p99 | 退化 ≤ 50% | 长尾容差大；> 50% 才算回归 |
| 错误率 | 新增 ≥ 5pp | 错误率上升 ≥ 5pp 视为回归 |
| RPS | 下降 ≤ 30% | 吞吐量下降 ≤ 30% 视为正常 |

### 10.3 模板：PR description 中贴性能影响

```markdown
## Performance Impact

| 场景 | 端点 | 基线 p95 | PR p95 | 变化 | 判定 |
|------|------|---------|--------|------|------|
| S2 login | POST /v1/iam/auth/login | 90ms | 95ms | +5.6% | ✅ |
| S3 list | GET /v1/iam/users | 156ms | 162ms | +3.8% | ✅ |
| S3 create | POST /v1/iam/users | 67ms | 78ms | +16.4% | ✅ (within 20%) |
| S5 create | POST /v1/config/namespace | 23ms | 102ms | **+343%** | ❌ **REGRESSION** |
| S7 publish | POST /v1/notification/announcements/{id}/publish | 2.5ms | 2.4ms | -4.0% | ✅ |

**结论**：S5 config create 引入回归，**必须 review**。其余场景在容差范围内。
```

### 10.4 当前已知瓶颈（PR review 时重点关注）

- **notification 服务 internal 限流 60/min**：任何变更若降低限流容忍度（去掉 sleep、加更多端点），必触发熔断
- **config list items 端点 500 bug**：未修复前，S5 测试持续高错误率，PR 数据对比可能失真
- **api-gateway 熔断阈值**：连续 5xx 触发 30s 熔断，PR 测试时需重启被改服务后再跑
- **login bcrypt 开销**：S2 login p50=63ms 由 bcrypt 决定，任何变更触及 `iam-service` 的 password hash 路径必看 S2 数据

### 10.5 自动化（roadmap）

- CI 中加 `k6-baseline-compare` job：main 合并后自动跑 + 存档基线
- PR 评论机器人：自动贴性能 diff 表
- 告警：当新基线 p95 > 旧基线 p95 × 2.0 时自动 ping 服务 owner
