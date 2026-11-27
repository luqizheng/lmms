# 12. 安全、运维与能力边界

## 12.1 凭证管理

- 生产凭证存放于 secret manager 或受保护部署环境，不提交 Git。
- 用户 access/refresh token 不写日志；服务凭证和 bootstrap token 按最高敏感级别保护。
- service-jwt 只用于其 audience 指定的服务。
- 凭证轮换应有责任人、时间窗口、验证和回滚计划。
- 任何一次性明文 secret 只在创建/轮换成功时安全保存；不要期待后续 API 能再次取回明文。

## 12.2 Gateway 与限流

API Gateway 是公共入口，承担 JWT 验签、黑名单/权限版本检查、路由和集中限流。限流规则在部署配置中维护，认证端点可设置更严格阈值。收到 429 时遵循 `Retry-After` 并使用有界指数退避；不要用并发重试放大流量。

Gateway route 配置与 rate limit 配置属于部署事实，不能只根据服务 OpenAPI 推断公共路由是否开放。

## 12.3 审计与可观测性

平台使用 OpenTelemetry trace context 传播请求链路；部署可配 Prometheus、Grafana、Jaeger 和 OTel Collector。排查问题时保留 request_id/trace_id、时间范围、服务名和错误码。

安全日志不要记录密码、token、secret、完整预签名 URL 或敏感文件内容。审计记录访问也应受权限控制。

## 12.4 文件数据备份与恢复

文件元数据数据库与对象存储必须作为一个逻辑整体管理：

- 同时备份数据库和对象存储。
- 恢复时校验 file ID、对象键、size、hash。
- 多实例生产优先使用共享 S3-compatible 后端。
- 不要把容器临时目录当作持久化存储。
- 对象删除失败、数据库提交失败等部分失败应有重试和巡检流程。

## 12.5 生产环境上线前检查

- [ ] Gateway 使用 HTTPS，证书自动续期。
- [ ] 公共只暴露 Gateway 和明确需要的 WebSocket 入口。
- [ ] IAM 首次注册策略已收紧，管理员账号和密码恢复流程已验证。
- [ ] 数据库、Redis、RabbitMQ、对象存储凭据已隔离并轮换。
- [ ] service-jwt audience grants 按最小权限配置。
- [ ] 文件大小、MIME、TTL、对象存储权限和备份恢复已测试。
- [ ] RabbitMQ outbox 消费失败、重复消费和重试有监控。
- [ ] 限流、trace、日志和审计查询已验证。
- [ ] Swagger/OpenAPI 暴露策略符合安全要求。
- [ ] 401/403/429/5xx 的客户端行为已验证。

## 12.6 当前能力边界速查

| 能力 | 当前状态/限制 |
|---|---|
| 用户登录、refresh、logout、会话 | IAM 已有接口，具体字段看 OpenAPI |
| 用户/角色/权限/租户 | IAM 能力；业务资源授权仍由业务服务负责 |
| 服务间身份 | bootstrap + service-jwt + audience grants；需最小授权 |
| 文件上传/下载/删除/元数据 | 内部 file-service；service-jwt only |
| 文件夹/移动/复制/版本/回收站 | 当前文件对象 API 未提供 |
| 通知公告、已读、未读数、WS | 已有公告能力；WS 协议需看当前实现 |
| 个人私信/任务待办/邮件短信 | 不应视为当前公告服务已提供 |
| 审计日志 | audit-service 提供日志能力；具体字段和权限看 OpenAPI |
| 通用 SDK | Rust 覆盖较广；Java 覆盖较窄 |
| 服务端口和公共路径 | 由 Gateway route 与部署配置决定 |

## 12.7 文档与代码核验来源

- `contracts/http-conventions.md`
- `contracts/permissions.yaml`
- `contracts/events.yaml`
- `services/*/openapi.yaml`
- `services/*/AGENTS.md`
- `sdk/rust/README.md`
- `sdk/java/README.md`
- `docs-site/guides/file-storage-usage.md`
- `docs-site/guides/java-integration.md`
- `docs-site/ai-context.md`

本手册应随实现更新。对于 WebSocket 消息格式、service-jwt 交换字段、Gateway rewrite 和各 SDK 构造函数等易变细节，优先引用当前代码而非复制可能过期的示例。
