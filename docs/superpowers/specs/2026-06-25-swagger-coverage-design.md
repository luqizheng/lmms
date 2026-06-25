# P2.1 Swagger UI 全覆盖设计

> 日期：2026-06-25
> 范围：audit-service / config-service / file-service / prue_backend 的 utoipa 注解 + ApiDoc 完整化
> 状态：执行中
> 关联：[overall-roadmap](../plans/2026-06-25-overall-roadmap.md) P2.1

---

## 1. 目标

把 P1.3 已经接好的 4 个服务（audit / config / file / prue_backend）的 `SWAGGER_ENABLED` 开关从「空 ApiDoc」升级到「完整 OpenAPI 文档」：
- 每个 handler 标注 `#[utoipa::path]`
- 每个 request / response 结构体 derive `ToSchema`
- `ApiDoc` 列出全部 `paths` + `components(schemas(...))`

完成标志：`SWAGGER_ENABLED=true` 启动任一服务后，浏览器打开 `/swagger-ui` 能看到所有端点 + schema 不为空。

---

## 2. 覆盖范围

### audit-service（4 个路由模块，~10 端点）
- operation: `POST /audit/operation`, `GET /audit/operation`
- login: `POST /audit/login`, `GET /audit/login`
- compliance: `GET /audit/export/compliance/operation/csv`, `GET /audit/export/compliance/operation/json`, `GET /audit/export/compliance/login/csv`
- retention: `GET/PUT /audit/retention/policy`, `GET /audit/retention/stats`, `POST /audit/retention/cleanup`, `GET /audit/retention/stage/{created_at}`

> compliance 当前 handler 用了 `Path(user_id)` 但路由上没有 `{user_id}` 占位符 — 是个潜在 bug，本 spec 不修，标注按现有路由签名。

### config-service（3 个路由模块，~12 端点）
- namespace: CRUD 5 个
- config: CRUD 5 个 + export/import 2 个
- watch: long-poll 1 个

### file-service（2 个路由模块，~6 端点）
- file: upload / list / download / delete / info（5 个）
- presign: create（1 个）

### prue_backend（1 个路由模块，~7 端点）
- menu-configs CRUD + publish + clone + get-async-routes

---

## 3. 标准模式

每个 handler 上方：
```rust
#[utoipa::path(
    <method>,
    path = "<full path including service prefix>",
    request_body = <T>,         // POST/PUT 才需要
    params(("id" = String, Path, ...)),  // 路径参数
    responses(
        (status = 200, description = "...", body = <T>),
        (status = 404, description = "..."),
    ),
    tag = "<group>"
)]
pub async fn handler(...) -> ... { ... }
```

每个 request / response struct derive `ToSchema`：
```rust
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ... { ... }
```

Query 类的 struct 加 `IntoParams` derive。

每个 `ApiDoc` 列出全部 paths + schemas + tags。

---

## 4. 处理决策

- **handler 改成 `pub`**：utoipa 要求 path 注解的函数可见，把现有私有 `async fn` 改 `pub async fn`
- **路径完整带 nest 前缀**：例如 audit 端点写 `/audit/operation` 而不是 `/operation`（matches what's exposed externally）
- **`ToSchema` 仅加在公开 schema**：sqlx `FromRow` 行结构体 + 请求/响应 DTO；不给内部 model 加
- **`IntoParams` + Query 结构体兼容**：utoipa 要求字段类型实现 `ToSchema`，已派生处统一加
- **跳过响应体复杂的 handler**：compliance 的 CSV 导出返回 raw string，response body 写 `String` 即可
- **跳过 Path 占位错配的 handler**：仅按路由签名标注，不修底层 bug（留作下一个独立任务）

---

## 5. 风险与缓解

| 风险 | 缓解 |
|---|---|
| utoipa 5 derive 在某些复杂 generic 上不工作 | 复杂 path 的 response 直接用 `serde_json::Value`（已是 axum 中的常用模式）|
| handler 改 pub 后命名冲突 | 4 个服务每个 routes 模块独立，无 cross-module 冲突 |
| ToSchema 给 sqlx FromRow 结构体可能编译失败 | utoipa 5 兼容；如失败回退到 wrapper |
| Cargo.lock 变化 | audit/config/file/prue 都已经在 P1.3 加好 utoipa 依赖 |

---

## 6. 不在范围

- api-gateway 聚合 UI（独立工程量）
- token-service / iam-service / notification-service：已有完整文档
- 修复 compliance 路由 Path 不匹配的 bug
- 后端 OpenAPI 提取脚本自动驱动 SDK 生成（属 P2.2）

---

## 7. 验收

- `node scripts/run-tests-docker.js audit-service config-service file-service prue_backend` 全 PASS
- 4 个 ApiDoc 都有非空 `paths` + 非空 `components`
- 启动 `SWAGGER_ENABLED=true` 后访问 `/swagger-ui` 能看到所有端点与 schema
