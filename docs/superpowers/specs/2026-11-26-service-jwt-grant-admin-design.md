# Service JWT Audience Grant 管理控制面设计（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01）

- 日期：2026-11-26
- 基线：main @ 55e379ff
- 任务书：WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01

## 0. 与任务书前提的差异（以当前代码为准）

任务书假设 LMMS 已有"SERVICE-JWT-AUDIENCE 相关工作"（运行时 owner→audience
授权检查、授权持久化、内置系统授权初始化）。经全仓库与 git 历史核查，
**这些在 main 上均不存在**：

- `services/token-service/src/models/service_jwt_grant.rs` 不存在；
- 无 `token_service_jwt_grants` 表（migrations 仅 V001–V003）；
- `POST /tokens/service-jwt`（`routes/service_jwt.rs`）仅校验 bootstrap
  token（`token_bootstrap_tokens`），对请求的 `aud` 无任何授权检查——
  持有任意有效 bootstrap token 的服务可为自己请求任意 audience。

因此本任务从零建立完整控制面：**迁移 + 运行时授权检查 + 系统授权 seed +
双入口管理 API + 审计 + SDK**。

## 1. 数据模型（V004，mysql + postgres 双方言）

### `token_service_jwt_grants`

| 列 | 类型 | 说明 |
|---|---|---|
| owner | VARCHAR(64) | bootstrap owner（与 `token_bootstrap_tokens.owner` 同字符集 `[a-zA-Z0-9_-]`） |
| audience | VARCHAR(64) | 目标服务名（同上字符集） |
| status | VARCHAR(16) | `active` / `disabled` |
| system_managed | BOOLEAN | 内置 seed 的保护行 |
| created_at / updated_at | TIMESTAMP | |

PK `(owner, audience)`；索引 `status`。应用层校验字符集（SQL 参数化，无注入面）。

### `token_service_jwt_grant_audit`（append-only）

id, actor_id, actor_type(`service`/`user`), owner, audience,
action(`create`/`enable`/`disable`/`delete`), before_status, after_status,
request_id, result(`success`/`denied`/`failed`), detail, occurred_at。

## 2. 运行时授权检查（`routes/service_jwt.rs`）

bootstrap 校验成功后追加：

1. 查 `token_service_jwt_grants`（owner=caller, audience=req.aud）；
2. `status='active'` → 放行；
3. 行不存在或 `disabled` → 403（不区分两种拒绝原因，避免探测）；
4. **DB 错误 → 500，fail-closed**。

行为变更说明：这是本任务的核心新增能力。存量部署升级后，未 seed 授权的
owner→audience 组合将被拒绝（默认拒绝），由 §3 seed 与管理 API 恢复。

## 3. 系统授权初始化（seed）

`models/service_jwt_grant.rs::seed_system_grants`（main.rs 启动时调用，在
bootstrap seed 之后）：

- grants 表为空时：为 `DEFAULT_DEV_BOOTSTRAP` 的每个 owner × 全部 7 个服务
  audience 建 `active + system_managed=true` 行（dev 开箱即用，幂等）；
- env `SERVICE_JWT_GRANT_SEED`（`owner:audience` 逗号分隔）→ 按行补插
  `system_managed=false`（生产声明式初始化入口，幂等，任何时候生效）；
- 格式非法 → 拒绝启动（fail-closed，与 bootstrap seed 语义一致）。

## 4. 双管理入口（共用 ops 业务层）

参考 `bootstrap_token_admin.rs` / `bootstrap_token_web.rs` 架构：

| 入口 | 路径 | 鉴权 |
|---|---|---|
| service | `/admin/service-jwt-grants*` | `verify_aud_principal_middleware("token-service")` |
| web | `/v1/admin/service-jwt-grants*` | user-jwt `Principal` + `require_perm` |

端点（两入口一致）：

- `GET  …/service-jwt-grants?owner=&audience=&status=&page=&size=` → `{items,total,page,size}`
- `GET  …/service-jwt-grants/{owner}/{audience}`
- `POST …/service-jwt-grants` `{owner, audience}` → 200；重复 409
- `PUT  …/service-jwt-grants/{owner}/{audience}/status` `{status}`；非法 400
- `DELETE …/service-jwt-grants/{owner}/{audience}` → 204；不存在 404

业务规则（ops 层，两入口唯一实现）：

- owner/audience 校验：1..=64，`[a-zA-Z0-9_-]`（与 bootstrap `validate_owner` 一致）；
- create：请求体不接受 `system_managed`（无该字段；DB 恒写 false）；
  并发重复创建由 PK 唯一约束兜底 → 409（先查后插 + 冲突映射）；
- set_status：仅 `active|disabled`；条件 UPDATE `AND status <> ?` 防并发覆盖
  /TOCTOU；行不存在 404；
- delete：`system_managed=true` → 409（系统授权保护）；不存在/已删 → 404；
- 分页：page ≥ 1，size 1..=100，越界 400；稳定排序 `ORDER BY owner, audience`；
- 错误响应不泄露 SQL / 内部信息（`internal()` 模式，仅 tracing 记录）。

## 5. 权限码（IAM 契约）

`contracts/permissions.yaml` 新增（与 `TOKEN_BOOTSTRAP_*` 同区）：

- `TOKEN_SERVICE_JWT_GRANT_READ` / `_CREATE` / `_UPDATE` / `_DELETE`

运行时执行：web 入口逐 handler `Principal::has_permission`（user-jwt
permissions claim，签发链路来自 IAM）。`node scripts/contracts-gen.js`
重生成 `libs/authz/src/perms.rs` 与 TS contracts（禁手改生成文件）。

## 6. 审计

- 每个成功变更 + 系统保护拒绝（409）+ 权限拒绝（403，web 入口）写一行审计；
  `request_id` 取 `X-Request-Id` header（缺省生成 uuid）；
- 不记录任何 token/JWT/凭据明文；
- **缺口说明**：token-service 无 RabbitMQ/outbox 依赖，无法进 audit-service
  跨服务审计链路；bootstrap 管理面现状仅有 tracing 日志。本任务采用**同库
  本地审计表**作为最小扩展（字段对齐 `audit_events` 行形，后续可平移）。
  变更与审计写入非同一事务（`db_*!` 宏不支持事务 executor）：审计写失败
  → 500 + error 日志（变更已生效，客户端可见失败）。报告中列为已知限制。

## 7. OpenAPI / 网关

- 全部 10 个 handler（5×2 入口）注册 `#[utoipa::path]` + `openapi.rs` schemas；
  跑 `node scripts/dump-openapi-specs.js` 重生成 `services/token-service/openapi.yaml`；
- `examples/{docker-deploy,docker-deploy-full}/config/api-gateway/routes/token.router.toml`
  新增 `/v1/admin/service-jwt-grants`（+`/{rest}`）转发（网关不验签下游验）。

## 8. SDK（代码与测试就绪，不发版）

| 生态 | 包 | 变更 |
|---|---|---|
| Rust | `sdk/rust/token-service-sdk` | model + client 5 方法 + wiremock 测试 |
| Java | `sdk/java/digua-token-client-sdk` | GrantInfo + 同步/异步方法 |
| npm | `sdk/iam-services` | `tokens.ts` 加 `ServiceJwtGrantsApi`（走 web 入口，复用 auth 注入）+ vitest |

版本号不动（各生态版本策略独立；发布另行任务）。npm 侧无独立 token-service
包，token 域已落在 iam-services 的 TokensApi，故随包扩展。

## 9. 测试矩阵

- **单元**（`#[cfg(test)]`，纯函数）：owner/audience 校验、状态枚举、分页
  边界、seed 解析（合法/非法/env）、系统保护判定。
- **集成**（MySQL，共享库/testcontainers）：未认证 401、错 aud service-jwt
  401、无权限 user-jwt 403、有权限全流程、create→签发成功、disable→签发
  403、delete→签发 403、重复 409、重复 delete 404、system_managed 保护、
  并发创建仅一次成功、两入口结果一致、分页越界 400、DB 挂 → 签发 500。
- **PostgreSQL**：扩展 `pg_roundtrip_test.rs`（grants 建表 + roundtrip）。
- **SQLite**：token-service 无 sqlite 迁移/runtime（`db.rs` 仅
  Mysql/Postgres），CI 矩阵亦无 → 不适用，报告中说明。

## 10. 明确不做

- 不改 bootstrap token 签发语义与 `token_bootstrap_tokens` 结构；
- 不改已发布迁移（新增 V004）；
- 不执行任何 Registry 发布 / 生产迁移 / PR 合并；
- 不引入 MQ 依赖。
