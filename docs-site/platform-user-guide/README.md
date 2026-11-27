# LMMS 平台使用手册

> 面向 LMMS 使用方、业务系统集成方和 AI-PM 等上层平台。以当前 Rust 服务实现和 OpenAPI 契约为准，Java SDK 为补充。
>
> 文档基线：2026-11-27。接口以部署版本的 OpenAPI 为最终依据。

## 阅读导航

| 章节 | 内容 | 主要读者 |
|---|---|---|
| [01 平台概览与接入准备](./01-overview.md) | 平台边界、服务地图、接入方式、首次联调 | 所有人 |
| [02 API 入口与通用约定](./02-api-conventions.md) | Gateway、路径、响应、分页、时间、错误与 OpenAPI | 集成开发 |
| [03 用户登录与会话](./03-user-authentication.md) | 注册、登录、刷新、登出、密码、会话管理 | 前端 / 后端 |
| [04 用户、角色、权限与租户](./04-iam-rbac-tenancy.md) | IAM、RBAC、租户隔离、权限码、UI 权限呈现 | 管理后台 / AI-PM |
| [05 服务身份与服务间鉴权](./05-service-authentication.md) | bootstrap token、service-jwt、audience、服务间调用 | 后端 / 平台运维 |
| [06 文件对象服务](./06-file-objects.md) | 上传、下载、预签名、归属、存储、生命周期 | 文件 UI / 业务服务 |
| [07 通知与实时推送](./07-notifications.md) | 公告、接收范围、已读、未读数、WebSocket | 通知中心 UI |
| [08 审计与领域事件](./08-audit-events.md) | 操作/登录审计、RabbitMQ、outbox、事件订阅 | 管理后台 / 集成开发 |
| [09 Rust SDK（主路径）](./09-rust-sdk.md) | crate、安装、客户端、鉴权、错误处理 | Rust 开发 |
| [10 Java SDK（补充路径）](./10-java-sdk.md) | Maven 坐标、模块、调用示例、版本注意 | Java 开发 |
| [11 AI-PM 集成指南](./11-ai-pm-integration.md) | UI 能力映射、文件/通知/鉴权集成、能力缺口 | AI-PM 产品与研发 |
| [12 安全、运维与能力边界](./12-security-operations.md) | 凭证、备份、限流、追踪、生产核验 | 运维 / 安全 |

## 推荐阅读路径

- **使用 LMMS 的业务系统**：01 → 02 → 03 → 04 → 按需阅读 06/07/08 → 09。
- **Rust 服务端集成**：01 → 02 → 05 → 06/07/08 → 09。
- **Java 应用集成**：01 → 02 → 03/04 → 10。
- **AI-PM 设计与实现**：01 → 03 → 04 → 05 → 06 → 07 → 08 → 11 → 12。
- **部署和安全审查**：02 → 05 → 08 → 12。

## 重要原则

1. 对外业务客户端统一访问 API Gateway；不要将容器内部端口当成公共 API。
2. 用户 JWT 与 service-jwt 是不同信任域。用户操作使用用户身份；服务间操作使用明确 audience 的服务身份。
3. IAM 负责身份、角色和权限；业务服务仍须检查资源级授权和业务归属。
4. file-service 是内部文件对象服务，不是通用网盘；业务服务负责用户授权和业务关联。
5. 当前 notification-service 主要提供公告和实时推送能力，不等同于通用站内信/消息队列产品。
6. OpenAPI 是接口形状的重要来源，但安全策略、Gateway 路由和实际部署配置也必须一起核对。
7. 不要把本文未列出的功能推断为已支持。章节中的“当前不支持/需单独设计”是明确的集成边界。

## 代码与契约入口

- [仓库 README](../../README.md)
- [AI Context](../ai-context.md)
- [现有开发与运维手册](../user-manual/README.md)
- [HTTP 约定](../../contracts/http-conventions.md)
- [权限码字典](../../contracts/permissions.yaml)
- [领域事件字典](../../contracts/events.yaml)
- [Rust SDK](../../sdk/rust/README.md)
- [Java SDK](../../sdk/java/README.md)
- [服务 OpenAPI](../../services/)

## 维护规则

本文档是使用方指南，不替代代码契约。接口变更应同步更新服务 OpenAPI、SDK、Gateway 路由/权限契约和本手册。若本文与当前实现不一致，以当前代码和部署版本的 OpenAPI 为准，并应提交文档修正。
