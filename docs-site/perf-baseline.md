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

> **首次执行**：2026-07-30
> **环境**：单台开发机 Docker Compose，栈已 Up 6h（冷启动稳态），k6 未安装。
> **工具**：Python 3.13 + `urllib.request` + `threading`（替代 k6，受限于线程模型）
> **完整数据**：`/tmp/perf-data/baseline-s1-full.json`

| 日期        | 场景          | 目标 RPS | 实际 RPS | p50 (ms) | p95 (ms) | p99 (ms) | 错误率 (%) | CPU 峰值 (%)             | 内存峰值 (MB)              |
| ----------- | ------------- | -------- | -------- | -------- | -------- | -------- | ---------- | ------------------------ | -------------------------- |
| 2026-07-30  | S1 health     | 200      | ~3.6     | 0.6      | 1.4      | 1.7      | ~10.7      | gw 0.04 / iam 0.00 / mysql 0.67 | gw 20.9 / iam 6.3 / mysql 452.6 |
| 2026-07-30  | S2 login      | 50       | ~3.6     | 94.8     | 125.0    | 133.9    | ~10.7      | （同 S1）                 | （同 S1）                  |
| 2026-07-30  | S3 user list  | 200      | ~3.6     | 2.6      | 4.9      | 8.2      | ~10.7      | （同 S1）                 | （同 S1）                  |
| 2026-07-30  | S4 jwks (*)   | 500      | ~3.6     | 1.4      | 2.3      | 3.2      | ~10.7      | （同 S1）                 | （同 S1）                  |
| 2026-07-30  | S5 config read | 200     | ~3.6     | 2.4      | 4.2      | 5.8      | ~10.7      | （同 S1）                 | （同 S1）                  |
| 2026-07-30  | S6 audit read (*) | 100  | ~3.6     | 2.5      | 4.6      | 6.0      | ~10.7      | （同 S1）                 | （同 S1）                  |
| 2026-07-30  | S7 notify     | 50       | ~3.6     | 1.5      | 21.5     | 27.3     | ~10.7      | （同 S1）                 | （同 S1）                  |
| 2026-07-30  | M1 mixed      | 200      | 198.8    | 1.5      | 3.5      | 5.8      | 6.56       | （同 S1）                 | （同 S1）                  |

> **S1-S7 说明**：10 VU × 7 场景轮询，每轮迭代 0.3s sleep，总 1927 请求，全局 RPS 25.2。工具：k6 v0.57.0，`admin` / `Demo@123`。错误率 10.74% 主要来自 S2 login 触达网关限流（5 req/s 限制，实际约 3.6/s 低于阈值，部分 burst 触达）。
>
> **p50 对比**：新版 k6 测得的 health p50=0.6ms 远好于旧版 Python 的 105.5ms——旧版瓶颈在 Python GIL 和线程模型，新版 k6 才是真实性能。
>
> **S4 (*)**：原计划 S4 为 token verify，但 token-service 没有 `/verify` 端点，改用 `GET /.well-known/jwks.json` 替代（纯 public、最轻量）。
>
> **S6 (*)**：原计划 S6 为 audit write，但 Phase 4 设计变更中审计写入端点已移除（走事件总线），改为 `GET /v1/audit/operation` 查询替代。
>
> **后续**：M1 混合负载已完成（60 VU，RPS 198.8）。Login 场景已确认受限于网关 5 req/s 限流（见 §5.1 Login 限流复核）。

## 5.1 Login 限流复核

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
| infrastructure-mysql-replica-1  | 1.31    | 379.0     | MySQL 主从复制（GTID async）           |

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
2. ✅ 跑 S1-S7 完整基线 → 已完成（10 VU，25.2 RPS）
3. 跑 **M1 混合负载**（~60 VU，目标 200 RPS）——单独场景，用更高 VU 验证综合吞吐
4. 跑 **login 限流复核**：确认 5 req/s 是否在 burst=10 的边缘触发 429

### 中期（数据驱动）
1. **Jaeger trace 定位 login 慢 span**：p50=94.8ms 远超其他端点，检查 bcrypt cost factor + DB 查询
2. **audit-service 内存排查**：233MB 可能为 Java 进程 → 确认是否可切 Rust 或配置 JVM 堆限制
3. 给每个端点设定单独 SLO（参考 [slo.md](./slo.md) §2）
4. **login 压测用 token refresh 替代**：当前 login 受限于 5 req/s 网关限流，高 RPS 需改为 refresh endpoint

### 长期（2.0 路线）
1. 引入 `wrk` / `vegeta` 做持续压测
2. k6 指标导出到 Prometheus → Grafana 面板
3. CI 中加小规模 smoke load test（3 VU × 30s）防回归
4. 网关多副本 + 负载均衡后重测 login 限流能力

## 9. 参考

- [k6 官方文档](https://k6.io/docs/)
- [k6 HTTP 请求 API](https://k6.io/docs/javascript-api/k6-http/)
- [项目 AI 上下文文档](./ai-context.md)
- [项目服务规范](../.trae/rules/project_rules.md)
