# 微服务 Swagger UI 接入设计

- 分支：`feat/swagger-ui`
- 范围：`token-service`、`iam-service`
- 日期：2026-06-16

## 1. 背景

`token-service` 和 `iam-service` 的 `AGENTS.md` 已声明 Swagger 必备组件，但代码中：

- `services/token-service/src/main.rs` 仅声明空 `ApiDoc`，未挂 `SwaggerUi`
- `services/iam-service` 完全无 OpenAPI/Swagger 相关代码
- `Cargo.toml` 已声明 `utoipa = "4"` 与 `utoipa-swagger-ui = "4"`

需要补齐 Swagger UI，便于本地联调和契约对齐。

## 2. 目标

- 两服务挂载 `/swagger-ui` 与 `/api-docs/openapi.json`
- 通过 `SWAGGER_ENABLED` 环境变量控制启用，默认 true
- OpenAPI 增加 Bearer JWT 安全方案
- 核心路由（登录/刷新/吊销/菜单/用户 CRUD）补 `#[utoipa::path]` 注解

## 3. 架构

```
┌──── token-service (6001) ────┐    ┌──── iam-service (6002) ────┐
│  /api/tokens/*                │    │  /iam/auth/*                │
│  /api/v1/admin/*              │    │  /iam/menus/* …             │
│                               │    │                             │
│  if SWAGGER_ENABLED:          │    │  if SWAGGER_ENABLED:        │
│   /api-docs/openapi.json      │    │   /api-docs/openapi.json    │
│   /swagger-ui                 │    │   /swagger-ui               │
└───────────────────────────────┘    └─────────────────────────────┘
```

- 每服务独立 `ApiDoc`，仅聚合本服务 path/schema
- Bearer JWT SecurityScheme 由 `utoipa::Modify` 注入

## 4. 组件分解

### token-service

| 文件 | 改动 |
|---|---|
| `src/openapi.rs` (新) | 定义 `ApiDoc`，聚合 paths/schemas + `SecurityAddon` (Bearer JWT) |
| `src/lib.rs` | `pub mod openapi;` |
| `src/routes/access.rs` | handler 加 `#[utoipa::path]` |
| `src/routes/refresh.rs` | handler 加 `#[utoipa::path]` |
| `src/routes/revoke.rs` | handler 加 `#[utoipa::path]` |
| `src/routes/user_jwt.rs` | handler 加 `#[utoipa::path]` |
| `src/routes/admin.rs` | handler 加 `#[utoipa::path]` |
| `src/models/oauth_client.rs` 等 DTO | 加 `derive(ToSchema)` |
| `src/main.rs` | 删除空 `ApiDoc`；按 `SWAGGER_ENABLED` 合并 `SwaggerUi` |

### iam-service

| 文件 | 改动 |
|---|---|
| `src/openapi.rs` (新) | 同上 |
| `src/lib.rs` | `pub mod openapi;` |
| `src/routes/auth.rs` | login/refresh/logout/password/sessions 加 `#[utoipa::path]` |
| `src/routes/menus.rs` | list/create/get/update/delete 加 `#[utoipa::path]` |
| `src/routes/users.rs` | list/create/get/update/delete 加 `#[utoipa::path]` |
| `src/dto/auth.rs` `dto/menu.rs` `dto/user.rs` | 加 `derive(ToSchema)` |
| `src/routes/mod.rs` 或 `main.rs` | 按 `SWAGGER_ENABLED` 合并 `SwaggerUi` |
| `Cargo.toml` | 已有 `utoipa-swagger-ui`，无需改 |

### 公用约定

- 路径：`/swagger-ui`、`/api-docs/openapi.json`
- 开关：`SWAGGER_ENABLED=true|false`，缺省 true
- 安全方案：`bearer_auth` (HTTP Bearer, format=JWT)
- 受保护接口在 `#[utoipa::path]` 中加 `security(("bearer_auth" = []))`

## 5. 数据流

```
浏览器 ──GET /swagger-ui──▶ utoipa_swagger_ui (静态资源)
浏览器 ──GET /api-docs/openapi.json──▶ ApiDoc::openapi() 序列化
浏览器 ──Authorize Bearer <jwt>──▶ Try it out ──▶ 业务路由 (Authorization 头透传)
```

- OpenAPI JSON 在编译期由 utoipa 派生宏生成结构，运行期序列化
- Swagger UI 静态资源内嵌于 crate，不联网

## 6. 错误处理

- `SWAGGER_ENABLED` 解析异常：按默认 true 处理，不阻断启动
- ApiDoc 内 schema 引用缺失：编译期暴露
- 关闭时 `/swagger-ui` 与 `/api-docs/openapi.json` 均返回 404

## 7. 测试

- 集成测试 (`tests/`):
  - `SWAGGER_ENABLED` 不设：`GET /api-docs/openapi.json` 返回 200 且 JSON 含核心 path
  - `SWAGGER_ENABLED=false`：`GET /swagger-ui` 返回 404
- 编译：`cargo build -p token-service -p iam-service` 通过且无新增 warning
- 现有 e2e 不变

## 8. 范围外

- 其他服务（notification/audit/config/file/api-gateway）本期不动
- gateway 聚合多服务 OpenAPI 的能力本期不做
- 全量路由注解（仅核心路由）本期不做
