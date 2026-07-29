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
| 2026-07-30  | S1 health     | 200      | 91.2     | 105.5    | 116.6    | 122.4    | 0.00       | gw 10.44 / iam 2.71 / mysql 0.77 | gw 35.64 / iam 13.91 / mysql 342.7 |
| 待补        | S2 login      | 50       | -        | -        | -        | -        | -          | -                        | -                          |
| 待补        | S3 user list  | 200      | -        | -        | -        | -        | -          | -                        | -                          |
| 待补        | S4 token verify | 500    | -        | -        | -        | -        | -          | -                        | -                          |
| 待补        | S5 config read | 200     | -        | -        | -        | -        | -          | -                        | -                          |
| 待补        | S6 audit write | 100     | -        | -        | -        | -        | -          | -                        | -                          |
| 待补        | S7 notify     | 50       | -        | -        | -        | -        | -          | -                        | -                          |
| 待补        | M1 mixed      | 200      | -        | -        | -        | -        | -          | -                        | -                          |

> **S1 说明**：10 worker 并发持续 30s。实测 RPS 91.2 远低于目标 200（受 Python GIL + 线程数限制，k6 应能更高）。p50=105.5ms 显著高于理想健康检查延迟（< 50ms），说明 api-gateway 自身有 ~100ms baseline 处理开销（trace 注入 + 健康检查 handler）。
>
> **S2-S7 未跑**：需要安装 k6（`sudo apt-get install k6` 或 `brew install k6`）后用 `tests/load/baseline.js` 重跑。

## 6. 系统资源消耗

> 仅基于 S1 测试（6 个 3s 间隔采样）。S2-S7 补全后重写。

| 容器                            | CPU 均值 (%) | 内存均值 (MB) | CPU 峰值 (%) | 内存峰值 (MB) | 备注                                   |
| ------------------------------- | ------------ | ------------- | ------------ | ------------- | -------------------------------------- |
| infrastructure-api-gateway-1    | ~9.5         | ~34          | 10.44        | 35.64         | 单副本承担全部请求；trace 注入 + 路由 |
| infrastructure-iam-service-1    | ~2.5         | ~13          | 2.71         | 13.91         | 仅 health 检查路径，负载极轻           |
| infrastructure-mysql-1         | ~0.7         | ~340         | 0.77         | 342.7         | 单实例独占 340MB buffer pool           |
| Redis / RabbitMQ / Consul       | 未采样       | -             | -             | -             | S1 未触发 IO，可忽略                   |
| token-service / audit / config / file / notification / prue_backend | 未采样 | - | - | - | S1 不涉及，可后续补充 |

> **关键观察**：单台机器 16GB 内存下 MySQL 占 343MB 主要是 `innodb_buffer_pool_size` 默认分配。S1 90RPS 流量下整个网关+iam+db 总 CPU < 14%，单机仍可承担 10x 流量。

## 7. 瓶颈分析

> 首次执行观察（待 S2-S7 数据补全）

1. **API Gateway baseline 延迟 ~100ms**：健康检查理论上应该 < 5ms，但实测 p50=105ms 说明 gateway 自身处理有显著开销。建议：用最小化 handler 做对照测试（如 `wget` 直接命中 nginx），排除 OTel/trace 注入影响。
2. **Python 测试工具瓶颈**：实测 RPS 仅 91.2（目标 200）大概率受 GIL + 线程数限制，不反映真实性能上限。**k6 是唯一可信工具**——必须安装后再做完整基线。
3. **未观察到资源瓶颈**：CPU < 14%、内存 < 400MB。S2-S7 更高 RPS 场景才有意义。

## 8. 后续优化建议

> 首次基线数据不足以给具体建议，先记框架。

### 短期（k6 装好后立即做）
1. 安装 k6：`sudo apt-get install k6`（Linux）/ `brew install k6`（macOS）
2. 用 `tests/load/baseline.js` 重跑 S1，确认 RPS 能达到 200+
3. 跑 S2-S7（login / user list / token verify / config read / audit write / notification send）
4. 跑 M1 混合负载，验证 200 RPS 综合能力

### 中期（数据驱动）
1. 根据 S2-S7 的 p99 数据，识别最慢端点
2. 用 Jaeger trace 定位慢 span（DB 查询 / JWT 验签 / 序列化）
3. 给每个端点设定单独的 SLO 阈值（参考 [slo.md](./slo.md) §2）

### 长期（2.0 路线）
1. 引入 wrk / vegeta 等工具做持续压测
2. 加 Prometheus exporter 输出 k6 指标到 Grafana
3. 在 CI 中跑小规模 smoke load test 防回归

## 9. 参考

- [k6 官方文档](https://k6.io/docs/)
- [k6 HTTP 请求 API](https://k6.io/docs/javascript-api/k6-http/)
- [项目 AI 上下文文档](./ai-context.md)
- [项目服务规范](../.trae/rules/project_rules.md)
