# 01. 平台概览与接入准备

## 1.1 LMMS 是什么

LMMS（仓库历史名称包含 coder-micro-services）是一组以 Rust/Axum 为主的微服务能力，为独立业务系统提供身份、令牌、文件、通知、审计等基础服务。LMMS 不随附业务前端控制台；业务系统应在自己的前端和后端中集成 LMMS。

典型接入拓扑：

```text
业务前端 / 桌面端 / 外部业务后端
             |
             | HTTPS + JSON / WebSocket
             v
        API Gateway
             |
       Rust microservices
       |   |   |   |
      IAM Token File Notification Audit
             |
       MySQL / Redis / RabbitMQ / Object Storage
```

## 1.2 服务职责

| 服务 | 主要职责 | 使用方通常如何接入 |
|---|---|---|
| api-gateway | 唯一公共 HTTP 入口、JWT 验签、黑名单/权限版本校验、路由、限流、trace | 所有外部 HTTP 客户端 |
| iam-service | 用户、登录、会话、角色、权限、组织和租户 | 用户 API / 管理 API |
| token-service | JWT、JWKS、OAuth/API key、服务凭证相关能力 | 认证链路及受控管理 API |
| file-service | 文件元数据、字节存储、上传/下载、预签名 URL | 由可信业务服务调用 |
| notification-service | 公告、接收范围、已读状态、未读计数、WebSocket 推送 | 用户通知 UI / 管理端 |
| audit-service | 操作日志、登录日志和事件消费 | 管理端查询、审计集成 |
| prue_backend | 菜单/动态路由配置后端 | 需要其菜单配置能力的管理端 |
| outbox publishers | 将事务 outbox 事件发布到 RabbitMQ | 平台内部基础设施，不是客户端 API |

服务集合和状态会演进；请以部署版本的 compose、服务 README 和 OpenAPI 为准。

## 1.3 公共入口与网络边界

- 生产环境应给 API Gateway 配置 HTTPS 域名，例如 `https://api.example.com`。
- 本地开发常用 `http://localhost:3000`。
- 业务客户端不应直接访问 IAM、Token、File 等服务的容器端口。
- Docker 网络中的服务名和容器端口只用于平台内部通信，不是外部 API 地址。
- WebSocket 等长连接也应通过 Gateway 暴露的对应路由；必须核对部署的路由配置和反向代理升级头设置。

## 1.4 接入前需要准备什么

1. LMMS 环境地址（开发、测试、生产应分开）。
2. 业务系统标识及其部署归属。
3. 用户登录方式和用户生命周期策略。
4. 如需服务间调用，申请独立服务身份、bootstrap 凭证及最小 audience 授权。
5. 确认租户模型：单租户或多租户；确定业务数据中的 tenant_id 来源。
6. 确认文件存储后端、容量、备份和下载方式。
7. 确认通知接收人范围、推送连接和断线重连策略。
8. 确认审计留存和访问权限。

不要在浏览器代码、移动端包、桌面客户端配置文件或 Git 仓库中放置服务端密钥。

## 1.5 第一次联调建议

先验证 Gateway 健康状态，再走用户注册/登录流程，最后调用一个受保护的 IAM API。建议顺序：

```text
GET  <gateway>/v1/iam/auth/setup-status
POST <gateway>/v1/iam/auth/register       # 仅首次初始化窗口/允许注册时
POST <gateway>/v1/iam/auth/login
GET  <gateway>/<一个已授权的 IAM 查询端点>
```

实际路由以 `infrastructure/config/api-gateway/routes/` 和部署版本 OpenAPI 为准。不要把内部服务路径直接拼到 Gateway URL 上。

## 1.6 环境与配置约定

- 数据库方言、Redis、RabbitMQ、Consul、对象存储等由部署配置决定。
- 服务端配置和 secret 通过部署环境变量/secret 管理注入；不要将生产 secret 写入示例配置。
- 业务应用应将 Gateway base URL 作为环境配置，不要散落硬编码。
- 开发、测试、生产使用不同用户、租户、客户端凭据和存储桶。
