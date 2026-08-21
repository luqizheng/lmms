# 02. 架构总览

> 回答三个问题：有哪些服务、怎么分层、一次请求怎么走。
> 上次更新：2026-08-21（重写 §2.1-§2.5 以反映 registrator + 直接 HTTP 架构；删除过时的 §2.6 Consul Connect 整段；注册 / 服务发现请改读 [`docs-site/guides/service-registration.md`](../guides/service-registration.md)）

---

## 2.1 服务地图（Phase 5-A-2 之后）

```
         browser / SDK
               │  :8080 (frontend dev) / 浏览器直连 api-gateway :3000
               ▼
        ┌──────────────┐
        │   frontend   │  :80 in container, nginx 反代到 api-gateway:3000
        └──────┬───────┘
               │  HTTP（明文；Authorization 透传）
               ▼
   ┌──────────────────────────────────────────┐
   │ api-gateway 容器 :3000                   │
   │   bind 0.0.0.0:3000                      │
   │   egress_map → 直连下游容器 :808x        │
   └──────┬───────────────────────────────────┘
          │  HTTP（明文；Authorization: Bearer <jwt> 透传）
          ▼   ┌──────────────────────────────────────────────┐
   ┌────────────┴──────┐    ┌──────────────────────┐    ┌────┴───────────┐
   │ token-service     │    │ iam-service          │    │ notification-  │
   │ :8082             │    │ :8081                │    │ service :8083  │
   │ → 直连 iam:8081   │    │                      │    │ → iam:8081     │
   └───────────────────┘    └──────────────────────┘    └────────────────┘
   ┌──────────────────────┐    ┌──────────────────────┐    ┌────────────────┐
   │ audit-service        │    │ prue_backend         │    │ file-service   │
   │ :8084                │    │ :8087                │    │ :8082          │
   │ (事件消费 + 写入)    │    │ (菜单配置)           │    │ (文件存储)     │
   └──────────────────────┘    └──────────────────────┘    └────────────────┘
   ┌──────────────────────┐
   │ config-service       │
   │ :8081                │  ← 与 iam 容器内端口冲突，靠 host 端口 8085 区分
   └──────────────────────┘

   ┌────────────────────────────────────────┐
   │ 后台 worker（无 HTTP 入口）             │
   │   iam-outbox-publisher                 │  ← MySQL outbox → RabbitMQ
   │   notification-outbox-publisher        │  ← MySQL outbox → RabbitMQ
   └────────────────────────────────────────┘

   ┌─────────────────────┐     ┌─────────────────────────────┐
   │ registrator（项目级）│────▶│ consul (catalog + DNS)      │
   │ watch docker.sock   │     │ 8500 HTTP / 8502 gRPC       │
   │ 监听容器启停 + labels│     └─────────────────────────────┘
   └─────────────────────┘
```

**关键点**（以 `infrastructure/docker-compose-services.yml` + `service-registration.md` 为准）：

- 所有业务服务**直连 HTTP**，无 sidecar / 无 mTLS / 无 envoy。
- 业务服务**共享 Docker 网络 `superpowers`**，DNS 解析走 Docker 内置：`http://iam-service:8081`、`http://token-service:8082` 等。
- 业务服务端口是**容器内端口**（8081-8087 / 3000），与 compose `ports:` 字段的 host 端口是两回事；gateway / 跨服务调用走容器内端口，host 端口只为开发联调。
- 注册由项目级 `gliderlabs/registrator` 容器监听 `/var/run/docker.sock`，从业务容器的 `SERVICE_NAME` / `SERVICE_PORT` / `SERVICE_<PORT>_CHECK_HTTP` labels 自动注册到 Consul catalog（详见 [`docs-site/guides/service-registration.md`](../guides/service-registration.md)）。
- 业务进程零 Consul 依赖，**不调用 Consul HTTP API**；服务身份 / 跨服务身份仍走 `Authorization: Bearer <jwt>` 透传（见 `.trae/rules/auth-rules.md`）。
- 2 个 outbox publisher worker（iam、notification）无 HTTP listener，不打 `SERVICE_*` labels，不出现在 Consul catalog。
- `config-service` 与 `iam-service` 共享容器内端口 8081，靠 host 端口（config-service 用 8085）消歧，不影响 gateway 转发。

---

## 2.2 分层职责

| 层 | 职责 | 禁止 |
|----|------|------|
| **api-gateway** | JWT 验签（拉 token-service JWKS）、Redis 黑名单、限流、注入 trace context、按 `routes/*.router.toml` 代理到下游 | 业务逻辑 |
| **token-service** | 签发 / 刷新 / 吊销 JWT；JWKS 暴露 | 用户 / 角色 / 权限 CRUD |
| **iam-service** | 身份 + RBAC + 部门树；perm_epoch 直写 Redis（Phase 4.4）；事件 `iam.permission.changed` 经 outbox → RabbitMQ | 直接发 token（只签发 user-jwt 给浏览器调它的 `/tokens/user-jwt`）|
| **notification-service** | 公告 + WebSocket 推送；role/dept 接收人解析（Phase 5B-fu3 起改本地 `ntf_user_projection` 投影 + RabbitMQ 事件，零同步 HTTP）| 鉴权；直发邮件（走 email service）|
| **audit-service** | 审计事件落库 + RabbitMQ 消费（`audit.iam-events` 绑 `iam.#`，风控队列 `audit.login-risk` 绑 `iam.login.failed`）| 调业务服务 |
| **prue_backend** | pure-admin 菜单配置版本化（草稿 / 发布 / 克隆）| 关联 iam-service；用户 / 角色实体 |
| **file-service** | 文件存储 / 下载（本地 `STORAGE_PATH`）| 跨服务调用 |
| **config-service** | 通用配置中心（内网可信）| 跨服务调用 |
| **outbox publisher** | 轮询 `*_outbox_events` 表 → 发 RabbitMQ exchange `events`（事务性发件箱）| 对外暴露 HTTP |
| **frontend** | Vue 3 admin UI；nginx 反代到 api-gateway:3000 | 业务逻辑 |

**原则**：

- 业务服务之间**直接 HTTP 调用**（docker DNS + 容器内端口）；gateway 转发**用户 → 业务**这条链路；`Authorization: Bearer <jwt>` 在 gateway 入口后**整链路透传**到下游服务（包括 service-to-service 调用），下游按 `.trae/rules/auth-rules.md` 校验。
- 跨服务**审计 / 业务事件**走 RabbitMQ + transactional outbox（不在 HTTP 调用里塞副作用）；详见 `ai-context.md §7 pitfall 19`。
- Consul 仅承担服务发现 + 健康检查；不在流路径上做流量拦截（无 sidecar）。

---

## 2.3 典型请求链路（直连 HTTP，无 sidecar 跳）

```
1. 浏览器 POST /api/v1/iam/auth/login 带凭据
2. Nginx (:80) 代理到 api-gateway (:3000)；或浏览器直连 api-gateway
3. api-gateway 进程：
   a. trace 中间件：建根 span、抽取 W3C traceparent
   b. auth 中间件：JWT 验签（通过 jwks_url 拉 token-service 公钥）；Redis 黑名单 / perm_epoch 比对
   c. 限流中间件：登录接口 5 rps（routes/*.router.toml + rate-limits.toml）
   d. 代理 handler：routes/*.router.toml 匹配 → egress_map 查 → 直连 http://iam-service:8081/iam/auth/login
4. iam-service 进程：
   a. trace 中间件：通过 traceparent 接 span
   b. auth 中间件：校验凭据
   c. 颁发 access_token + refresh_token；落库 session
   d. 同事务展开 user_id 受影响权限 → 写 iam_outbox_events（Phase 4.4）
5. response 流回 api-gateway → 浏览器；响应头带 traceparent
6. span 上报 OTel collector（OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317）
7. Jaeger UI 展示完整链路：gateway → iam（无 sidecar 中间跳）
```

**Service-to-service（例 `token-service` 内部调 iam 取 user claims）**：

```
1. token-service 收到 POST /tokens/access 请求；handler 解析 username → 需要 iam 校验密码
2. token-service 内部 reqwest 直接打 http://iam-service:8081/iam/internal/users/:id/claims
   Authorization: Bearer <user-jwt>          ← 透传用户的 JWT（不是另签 service-jwt）
3. iam-service /iam/internal/* 允许 Audience::User（Phase 5-A-2 放宽，详见 auth-rules.md）
4. iam-service 返回用户 claims → token-service 颁发 token
```

**关键差异**：mesh 时代需要经 2 对 sidecar 跳（gateway-sidecar → gateway → iam-sidecar → iam），现在**只有 2 个 hop**（gateway → iam），省 400MB 内存（参见 spec §8.4）。

---

## 2.4 8 项服务标准（ADR-002）

每个 Rust 服务必须具备：

- [ ] Dockerfile
- [ ] `AGENTS.md`（本服务约定）
- [ ] `README.md`
- [ ] `tests/` 目录
- [ ] `/health` 端点
- [ ] `/metrics` 端点
- [ ] `migrations/` 目录（无 DB 可豁免）
- [ ] Swagger 配置（`SWAGGER_ENABLED` 环境开关）

CI 门禁：`node scripts/check-service-standard.js` 全过才允许 merge。

---

## 2.5 Swagger UI 覆盖

`SWAGGER_ENABLED=true` 时业务服务暴露 Swagger UI（在每个服务的 `main.rs` 里 `SwaggerUi::new("/swagger-ui")`）。`openapi.yaml` 由 `utoipa` 注解自动生成。

**端口配置**（来自 `infrastructure/docker-compose-services.yml` + 各服务 `config.rs` / `AGENTS.md`）：

| 服务 | docker compose 容器内端口 | docker compose host 端口 | `cargo run` 本地默认端口 | Swagger URL |
|------|--------------------------|------------------------|------------------------|-------------|
| api-gateway | 3000 | 3000 | 3000 | http://localhost:3000/api-docs/openapi.json |
| iam-service | 8081 | 9081¹ | 8081 | http://localhost:9081/swagger-ui |
| token-service | 8082 | 8082 | 4000² | http://localhost:8082/swagger-ui |
| notification-service | 8083 | 8083 | 8083 | http://localhost:8083/swagger-ui |
| audit-service | 8084 | 8084 | 8080² | http://localhost:8084/swagger-ui |
| prue_backend | 8087 | 8087 | 8080² | http://localhost:8087/swagger-ui |
| file-service | 8082 | 8086³ | 8082 | http://localhost:8086/swagger-ui |
| config-service | 8081 | 8085³ | 8081 | http://localhost:8085/swagger-ui |

> ¹ **iam-service compose host 端口 9081 是 Phase 5-A mesh 时代的 sidecar 外端口残留**（容器内 8081 未变），与 gateway / 其他服务不一致——gateway / registrator 走容器内 8081，host 直连开发联调则访问 9081。spec §7.3 规划的统一到 `8081:8081` 待落地（跟踪于 follow-up）。
>
> ² **token-service / audit-service / prue_backend 的 `cargo run` 默认端口**与 compose 不一致——code default 是 8080（prue_backend / audit-service）或 4000（token-service），compose 显式 `SERVER_PORT=...` 覆盖。开发时直跑 cargo 不带 env 会撞 8080 / 4000；带 compose env 后才落到 808x。
>
> ³ **file-service / config-service 的 host 端口**（8086 / 8085）是 Phase 5-A-2 spec §7.3 引入的（容器内端口与 iam / token 冲突，靠 host 端口消歧），与网关无关。

**改了 `#[utoipa::path]` 注解后**重新生成：

```bash
node scripts/dump-openapi-specs.js   # 8 个服务一起
```

CI 门禁：`node scripts/check-spec.js` 校验 yaml 合法性 + `git diff --exit-code` 检测漂移。

> **绝对不要手改 `openapi.yaml`** — 它是生成产物，下次 dump 会被覆盖。

---

## 2.7 后台 worker：outbox publisher

2 个 outbox publisher worker 跑事务性发件箱模式：

| Worker | 数据源 | 目标 | 备注 |
|--------|--------|------|------|
| `iam-outbox-publisher` | `iam_outbox_events` 表 | RabbitMQ exchange `events` | poll interval 1s, batch 100 |
| `notification-outbox-publisher` | `ntf_outbox_events` 表 | RabbitMQ exchange `events` | poll interval 1s, batch 100 |

> **归档**：活行 >24h 未投递 → 移入 `*_outbox_events_archive`；30 天后 purge。

**为什么需要 outbox**：跨服务事件要在**同一个 DB 事务**里投递，避免「业务写入成功但消息丢了」或「消息发了业务没写」。模式详见 [`docs-site/ai-context.md` §7 pitfall 19](../ai-context.md)。

这些 worker **不暴露 HTTP**、不参与服务发现（不打 `SERVICE_*` labels）、不在 Consul catalog 出现，只是单进程循环。

---

## 2.8 相关决策

- ADR-001 API Gateway 多文件路由配置
- ADR-002 8 项服务标准
- [Phase 5-A spec](../superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md) Consul Connect mTLS（已撤销；保留作历史考古，被 Phase 5-A-2 registrator 方案替代）
- 完整索引见 [`docs-site/decisions/`](../decisions/)