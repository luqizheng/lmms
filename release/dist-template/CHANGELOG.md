# Changelog

## v1.0.0 (2026-07-12)

### Added
- 完整离线分发包，含 docker-compose.yml 统一入口
- 8 个 Rust 微服务 + Vue 3 前端 + 可观测性栈
- Docker 镜像预加载脚本（air-gap 支持）
- JWT RS256 密钥自动生成脚本
- 端到端冒烟测试脚本（verify.sh）
- MySQL/Redis/Consul/RabbitMQ 基础设施 compose
- Prometheus + Grafana + Jaeger + OTEL 可观测性栈
- 统一 Makefile 入口（secrets/up/down/logs/verify/upgrade）

### Infrastructure
- MySQL 8.0（含 init-mysql 初始化脚本）
- Redis 7（密码认证）
- Consul 2.0（服务注册发现）
- RabbitMQ 3.13（management + definitions.json）
- Registrator（自动注册容器到 Consul）

### Services
| 服务 | 端口 |
|------|------|
| api-gateway | 8080 |
| config-service | 8081 |
| file-service | 8082 |
| token-service | 8083 |
| iam-service | 8084 |
| notification-service | 8085 |
| audit-service | 8086 |
| prue-backend | 8087 |

### Observability
- Prometheus v2.51.0（15d retention）
- Grafana 10.4.0（provisioned dashboards + datasources）
- Jaeger 1.55（OTLP 接收 + spanmetrics）
- OTEL Collector 0.96（traces → Jaeger, metrics → Prometheus）
- Consul-template 0.32（nginx canary 路由）

### Security
- JWT RS256 密钥自动生成（2048-bit RSA）
- 随机密码自动生成（32-char hex）
- bootstrap token 每服务独立
- 所有 secrets 不写入镜像，通过 .env 挂载

### Notes
- 前端为源码分发（不含 node_modules），含 pnpm-lock.yaml
- 镜像版本统一 v1.0.0
- 支持原地升级（保留 volumes）
- 数据目录 secrets/ 不提交 git