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

| 日期       | 场景                | 目标 RPS | 实际 RPS | p50 (ms) | p95 (ms) | p99 (ms) | 错误率 (%) | CPU (%) | 内存 (MB) |
| ---------- | ------------------- | -------- | -------- | -------- | -------- | -------- | ---------- | ------- | --------- |
| 待测试     | login               | 50       | -        | -        | -        | -        | -          | -       | -         |
| 待测试     | user list           | 200      | -        | -        | -        | -        | -          | -       | -         |
| 待测试     | token verify        | 500      | -        | -        | -        | -        | -          | -       | -         |
| 待测试     | config read         | 200      | -        | -        | -        | -        | -          | -       | -         |
| 待测试     | audit write         | 100      | -        | -        | -        | -        | -          | -       | -         |
| 待测试     | notification send   | 50       | -        | -        | -        | -        | -          | -       | -         |
| 待测试     | mixed               | 200      | -        | -        | -        | -        | -          | -       | -         |

## 6. 系统资源消耗

| 服务               | CPU 均值 (%) | 内存均值 (MB) | CPU 峰值 (%) | 内存峰值 (MB) | 备注                     |
| ------------------ | ------------ | ------------- | ------------ | ------------- | ------------------------ |
| iam-service        | 待填写       | 待填写        | 待填写       | 待填写        | 认证/授权核心服务         |
| token-service      | 待填写       | 待填写        | 待填写       | 待填写        | 令牌签发与校验            |
| config-service     | 待填写       | 待填写        | 待填写       | 待填写        | 配置读写服务              |
| audit-service      | 待填写       | 待填写        | 待填写       | 待填写        | 审计日志服务              |
| notification-svc   | 待填写       | 待填写        | 待填写       | 待填写        | 通知推送服务              |
| api-gateway        | 待填写       | 待填写        | 待填写       | 待填写        | 网关（JWT 验签 + 路由）   |
| MySQL              | 待填写       | 待填写        | 待填写       | 待填写        | 共享数据库                |
| Redis              | 待填写       | 待填写        | 待填写       | 待填写        | 缓存 / 黑名单             |
| RabbitMQ           | 待填写       | 待填写        | 待填写       | 待填写        | 消息队列                  |

## 7. 瓶颈分析

> 待首次基线测试完成后填写。

## 8. 后续优化建议

> 待首次基线测试完成后填写。

## 9. 参考

- [k6 官方文档](https://k6.io/docs/)
- [k6 HTTP 请求 API](https://k6.io/docs/javascript-api/k6-http/)
- [项目 AI 上下文文档](./ai-context.md)
- [项目服务规范](../.trae/rules/project_rules.md)
