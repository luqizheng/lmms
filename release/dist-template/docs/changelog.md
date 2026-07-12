# 变更日志

本项目的完整变更记录见仓库根目录 [`CHANGELOG.md`](./CHANGELOG.md)。

## 阅读说明

- 所有版本采用 **语义化版本**：`v主.次.补丁`
- Breaking changes 会在变更说明中标注
- 新增功能标注 **Added**，修复标注 **Fixed**，行为变更标注 **Changed**

## 版本历史

### v1.0.0 (2026-07-12)

**首个正式发布版本**

#### 新增
- 8 个 Rust 微服务 + Vue 3 前端完整源码
- Docker Compose 离线分发包
- API 网关限流与鉴权
- IAM 服务（用户/角色/权限）
- Token 服务（JWT RS256）
- Outbox Pattern 事件投递（iam-service / notification-service）
- 配置中心、文件服务、通知服务、审计服务
- Prueba 后端（MySQL + SQLite 双数据库）
- Prometheus + Grafana + Jaeger + OTel 可观测性栈
- Consul 服务注册与发现
- 一键部署脚本（`make up`）
- JWT 密钥自动生成（`make secrets`）
- 端到端冒烟测试（`make verify`）

#### 安全
- JWT RS256 密钥自动生成（2048-bit RSA）
- 随机密码自动生成（32-char hex）
- 每服务独立 bootstrap token

#### 基础设施
- MySQL 8.0（含 init SQL）
- Redis 7（密码认证）
- Consul 2.0
- RabbitMQ 3.13
- Registrator（自动注册容器到 Consul）
