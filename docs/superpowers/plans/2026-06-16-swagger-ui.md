# 微服务 Swagger UI 接入实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给 token-service 与 iam-service 接入 Swagger UI（`/swagger-ui` + `/api-docs/openapi.json`），由 `SWAGGER_ENABLED` 环境变量控制启用，OpenAPI 含 Bearer JWT 安全方案，覆盖核心路由。

**Architecture:** 每服务独立 `ApiDoc`（utoipa 派生），在 `main.rs` 启动时按 `SWAGGER_ENABLED` 决定是否 `.merge(SwaggerUi::new(...))`。`SecurityAddon` 通过 `utoipa::Modify` 注入 Bearer JWT。核心 handler 加 `#[utoipa::path]`，DTO 加 `derive(ToSchema)`。

**Tech Stack:** Rust / axum 0.8 / utoipa 4 / utoipa-swagger-ui 4 / Cargo workspace。

**Branch:** `feat/swagger-ui`（已创建）

---

## 文件结构

### token-service
- 新建：`services/token-service/src/openapi.rs` — `ApiDoc` 与 `SecurityAddon`
- 修改：`services/token-service/src/lib.rs` — 导出 `pub mod openapi;`
- 修改：`services/token-service/src/main.rs` — 删除内嵌空 `ApiDoc`，按开关挂 SwaggerUi
- 修改：`services/token-service/src/routes/access.rs` `refresh.rs` `revoke.rs` `user_jwt.rs` `admin.rs` — 加 `#[utoipa::path]`，DTO 加 `ToSchema`
- 新建：`services/token-service/tests/swagger_test.rs` — 集成测试

### iam-service
- 新建：`services/iam-service/src/openapi.rs` — `ApiDoc` 与 `SecurityAddon`
- 修改：`services/iam-service/src/lib.rs` — 导出 `pub mod openapi;`
- 修改：`services/iam-service/src/main.rs` — 按开关挂 SwaggerUi
- 修改：`services/iam-service/src/routes/auth.rs` `menus.rs` `users.rs` — 加 `#[utoipa::path]`
- 修改：`services/iam-service/src/dto/auth.rs` `dto/menu.rs` `dto/user.rs` — 加 `derive(ToSchema)`
- 新建：`services/iam-service/tests/swagger_test.rs` — 集成测试

### 公共约定
- 路径：`/swagger-ui`、`/api-docs/openapi.json`
- 开关：`SWAGGER_ENABLED=true|false`（缺省 true）
- 安全方案：`bearer_auth` (HTTP Bearer, format=JWT)

---

## Task 1: token-service DTO 加 ToSchema

**Files:**
- Modify: `services/token-service/src/routes/access.rs`
- Modify: `services/token-service/src/routes/refresh.rs`
- Modify: `services/token-service/src/routes/revoke.rs`
- Modify: `services/token-service/src/routes/user_jwt.rs`
- Modify: `services/token-service/src/routes/admin.rs`

- [ ] **Step 1: 给 access.rs 的 DTO 加 ToSchema**

修改 `services/token-service/src/routes/access.rs` 顶部：

```rust
use crate::service::{auth_code_svc, oauth_client_svc};
use crate::TokenServerState;
use argon2::{PasswordHash, PasswordVerifier};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct AccessRequest {
    pub grant_type: String,
    pub code: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccessResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}
```

- [ ] **Step 2: refresh.rs 同样处理**

```rust
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}
```

- [ ] **Step 3: revoke.rs 同样处理**

```rust
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct RevokeRequest {
    pub token_jti: String,
    pub expires_in: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RevokeResponse {
    pub success: bool,
}
```

- [ ] **Step 4: user_jwt.rs 同样处理**

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct UserJwtRequest {
    pub user_id: String,
    pub user_code: String,
    #[serde(default)]
    pub claims: Value,
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserJwtResponse {
    pub access_token: String,
    pub expires_in: u64,
}
```

- [ ] **Step 5: admin.rs DTO 加 ToSchema**

为 `ListQuery` `ClientDto` `ListResponse` `CreateRequest` `CreateResponse` `UpdateRequest` `ResetSecretResponse` 全部加 `, ToSchema` 与 `use utoipa::ToSchema;`，例如：

```rust
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ListQuery {
    pub keyword: Option<String>,
    pub status: Option<String>,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_size")]
    pub size: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClientDto {
    pub client_id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub grant_types: Vec<String>,
    pub access_token_ttl_sec: i64,
    pub refresh_token_ttl_sec: i64,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

其他类型按相同模式补齐。

- [ ] **Step 6: 编译校验**

Run: `cargo build -p token-service`
Expected: 编译通过（DTO 仅加 derive，不会破坏现有逻辑）

- [ ] **Step 7: Commit**

```bash
git add services/token-service/src/routes
git commit -m "feat(token-service): add ToSchema derives on core DTOs"
```

---

## Task 2: token-service 路由加 #[utoipa::path]

**Files:**
- Modify: `services/token-service/src/routes/access.rs`
- Modify: `services/token-service/src/routes/refresh.rs`
- Modify: `services/token-service/src/routes/revoke.rs`
- Modify: `services/token-service/src/routes/user_jwt.rs`
- Modify: `services/token-service/src/routes/admin.rs`

- [ ] **Step 1: access.rs handler 加注解**

在 `pub async fn handle_access(...)` 上方加：

```rust
#[utoipa::path(
    post,
    path = "/api/tokens/access",
    request_body = AccessRequest,
    responses(
        (status = 200, description = "issued tokens", body = AccessResponse),
        (status = 400, description = "bad request"),
        (status = 401, description = "invalid client"),
    ),
    tag = "tokens"
)]
pub async fn handle_access(
```

- [ ] **Step 2: refresh.rs handler 加注解**

```rust
#[utoipa::path(
    post,
    path = "/api/tokens/refresh",
    request_body = RefreshRequest,
    responses(
        (status = 200, description = "refreshed tokens", body = RefreshResponse),
        (status = 401, description = "invalid or expired"),
    ),
    tag = "tokens"
)]
pub async fn handle_refresh(
```

- [ ] **Step 3: revoke.rs handler 加注解**

```rust
#[utoipa::path(
    post,
    path = "/api/tokens/revoke",
    request_body = RevokeRequest,
    responses(
        (status = 200, description = "revoked", body = RevokeResponse),
    ),
    tag = "tokens"
)]
pub async fn handle_revoke(
```

- [ ] **Step 4: user_jwt.rs handler 加注解**

```rust
#[utoipa::path(
    post,
    path = "/api/v1/tokens/user-jwt",
    request_body = UserJwtRequest,
    responses(
        (status = 200, description = "user jwt issued", body = UserJwtResponse),
    ),
    tag = "tokens"
)]
pub async fn handle_user_jwt(
```

- [ ] **Step 5: admin.rs 五个 handler 加注解**

`handle_list`：
```rust
#[utoipa::path(
    get,
    path = "/api/v1/admin/oauth-clients",
    params(ListQuery),
    responses((status = 200, body = ListResponse)),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_list(
```

`handle_create`：
```rust
#[utoipa::path(
    post,
    path = "/api/v1/admin/oauth-clients",
    request_body = CreateRequest,
    responses(
        (status = 200, body = CreateResponse),
        (status = 400, description = "bad request"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_create(
```

`handle_update`：
```rust
#[utoipa::path(
    put,
    path = "/api/v1/admin/oauth-clients/{client_id}",
    params(("client_id" = String, Path,)),
    request_body = UpdateRequest,
    responses(
        (status = 200, body = ClientDto),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_update(
```

`handle_delete`：
```rust
#[utoipa::path(
    delete,
    path = "/api/v1/admin/oauth-clients/{client_id}",
    params(("client_id" = String, Path,)),
    responses(
        (status = 204, description = "deleted"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_delete(
```

`handle_reset_secret`：
```rust
#[utoipa::path(
    post,
    path = "/api/v1/admin/oauth-clients/{client_id}/reset-secret",
    params(("client_id" = String, Path,)),
    responses(
        (status = 200, body = ResetSecretResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_reset_secret(
```

- [ ] **Step 6: 编译校验**

Run: `cargo build -p token-service`
Expected: 编译通过

- [ ] **Step 7: Commit**

```bash
git add services/token-service/src/routes
git commit -m "feat(token-service): annotate core routes with utoipa::path"
```

---

## Task 3: token-service ApiDoc 与 main 集成

**Files:**
- Create: `services/token-service/src/openapi.rs`
- Modify: `services/token-service/src/lib.rs`
- Modify: `services/token-service/src/main.rs`

- [ ] **Step 1: 创建 openapi.rs**

写入 `services/token-service/src/openapi.rs`：

```rust
use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::routes::{access, admin, refresh, revoke, user_jwt};

pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi
            .components
            .get_or_insert_with(utoipa::openapi::Components::new);
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    info(title = "token-service", description = "Token issuance and OAuth client admin"),
    paths(
        access::handle_access,
        refresh::handle_refresh,
        revoke::handle_revoke,
        user_jwt::handle_user_jwt,
        admin::handle_list,
        admin::handle_create,
        admin::handle_update,
        admin::handle_delete,
        admin::handle_reset_secret,
    ),
    components(schemas(
        access::AccessRequest,
        access::AccessResponse,
        refresh::RefreshRequest,
        refresh::RefreshResponse,
        revoke::RevokeRequest,
        revoke::RevokeResponse,
        user_jwt::UserJwtRequest,
        user_jwt::UserJwtResponse,
        admin::ListQuery,
        admin::ListResponse,
        admin::ClientDto,
        admin::CreateRequest,
        admin::CreateResponse,
        admin::UpdateRequest,
        admin::ResetSecretResponse,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "tokens", description = "Token issuance"),
        (name = "admin", description = "OAuth client admin"),
    )
)]
pub struct ApiDoc;
```

- [ ] **Step 2: lib.rs 导出 openapi**

修改 `services/token-service/src/lib.rs`，在 `pub mod store;` 之后加：

```rust
pub mod config;
pub mod db;
pub mod issuer;
pub mod models;
pub mod openapi;
pub mod routes;
pub mod secret;
pub mod service;
pub mod store;
```

- [ ] **Step 3: 修改 main.rs 挂载 SwaggerUi**

修改 `services/token-service/src/main.rs`：

删除：
```rust
use utoipa::{OpenApi, ToSchema};
```
和
```rust
    #[derive(OpenApi)]
    #[openapi()]
    struct ApiDoc;
```

在文件头加：
```rust
use token_service::openapi::ApiDoc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
```

将构建 router 的代码改为：

```rust
    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);

    let mut app = axum::Router::new()
        .route(
            "/health",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .route("/metrics", axum::routing::get(metrics))
        .route(
            "/api/tokens/access",
            axum::routing::post(routes::access::handle_access),
        )
        .route(
            "/api/tokens/refresh",
            axum::routing::post(routes::refresh::handle_refresh),
        )
        .route(
            "/api/tokens/revoke",
            axum::routing::post(routes::revoke::handle_revoke),
        )
        .route(
            "/api/tokens/resource",
            axum::routing::post(routes::resource::handle_resource),
        )
        .route(
            "/api/tokens/.well-known/jwks.json",
            axum::routing::get(routes::jwks::handle_jwks),
        )
        .route(
            "/.well-known/jwks.json",
            axum::routing::get(routes::jwks::handle_jwks),
        )
        .route(
            "/api/api-keys",
            axum::routing::post(routes::api_key::handle_create_api_key),
        )
        .route(
            "/api/v1/tokens/user-jwt",
            axum::routing::post(routes::user_jwt::handle_user_jwt),
        )
        .route(
            "/api/v1/admin/oauth-clients",
            axum::routing::get(routes::admin::handle_list)
                .post(routes::admin::handle_create),
        )
        .route(
            "/api/v1/admin/oauth-clients/{client_id}",
            axum::routing::put(routes::admin::handle_update)
                .delete(routes::admin::handle_delete),
        )
        .route(
            "/api/v1/admin/oauth-clients/{client_id}/reset-secret",
            axum::routing::post(routes::admin::handle_reset_secret),
        )
        .with_state(state);

    if swagger_enabled {
        app = app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", ApiDoc::openapi()),
        );
        tracing::info!("swagger ui enabled at /swagger-ui");
    }
```

注意：`app` 由 `let app = ...` 改为 `let mut app = ...`。

- [ ] **Step 4: 构建校验**

Run: `cargo build -p token-service`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add services/token-service/src/openapi.rs services/token-service/src/lib.rs services/token-service/src/main.rs
git commit -m "feat(token-service): mount swagger-ui under SWAGGER_ENABLED flag"
```

---

## Task 4: token-service 集成测试

**Files:**
- Create: `services/token-service/tests/swagger_test.rs`

- [ ] **Step 1: 写测试文件**

写入 `services/token-service/tests/swagger_test.rs`：

```rust
use token_service::openapi::ApiDoc;
use utoipa::OpenApi;

#[test]
fn openapi_contains_core_paths() {
    let doc = ApiDoc::openapi();
    let json = serde_json::to_value(&doc).expect("serialize openapi");
    let paths = json
        .get("paths")
        .and_then(|v| v.as_object())
        .expect("paths object");

    assert!(paths.contains_key("/api/tokens/access"));
    assert!(paths.contains_key("/api/tokens/refresh"));
    assert!(paths.contains_key("/api/tokens/revoke"));
    assert!(paths.contains_key("/api/v1/tokens/user-jwt"));
    assert!(paths.contains_key("/api/v1/admin/oauth-clients"));
}

#[test]
fn openapi_has_bearer_security_scheme() {
    let doc = ApiDoc::openapi();
    let json = serde_json::to_value(&doc).expect("serialize openapi");
    let scheme = json
        .pointer("/components/securitySchemes/bearer_auth")
        .expect("bearer_auth scheme present");
    assert_eq!(scheme.get("type").and_then(|v| v.as_str()), Some("http"));
    assert_eq!(scheme.get("scheme").and_then(|v| v.as_str()), Some("bearer"));
    assert_eq!(scheme.get("bearerFormat").and_then(|v| v.as_str()), Some("JWT"));
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test -p token-service --test swagger_test`
Expected: 两个测试通过

- [ ] **Step 3: Commit**

```bash
git add services/token-service/tests/swagger_test.rs
git commit -m "test(token-service): verify openapi paths and bearer scheme"
```

---

## Task 5: iam-service DTO 加 ToSchema

**Files:**
- Modify: `services/iam-service/src/dto/auth.rs`
- Modify: `services/iam-service/src/dto/menu.rs`
- Modify: `services/iam-service/src/dto/user.rs`

- [ ] **Step 1: dto/auth.rs 加 ToSchema**

修改顶部 use 与每个结构体 derive：

```rust
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct LoginRequest {
    #[validate(length(min = 1, max = 100))]
    pub username: String,
    #[validate(length(min = 6, max = 128))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RefreshRequest {
    #[validate(length(min = 1))]
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 6, max = 128))]
    pub old_password: String,
    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ResetPasswordRequest {
    #[validate(length(min = 1))]
    pub username: String,
    #[validate(length(min = 1))]
    pub code: String,
    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub user: UserInfo,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub nickname: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionInfo {
    pub id: String,
    pub user_id: String,
    pub client_ip: String,
    pub user_agent: Option<String>,
    pub created_at: String,
    pub expires_at: String,
    pub is_current: bool,
}
```

- [ ] **Step 2: dto/menu.rs 加 ToSchema**

```rust
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateMenuRequest {
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(length(min = 1, max = 100))]
    pub code: String,
    #[validate(length(min = 1, max = 255))]
    pub path: String,
    pub component: Option<String>,
    pub icon: Option<String>,
    pub sort_order: Option<i32>,
    pub visible: Option<bool>,
    pub permission_id: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateMenuRequest {
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub code: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub path: Option<String>,
    pub component: Option<String>,
    pub icon: Option<String>,
    pub sort_order: Option<i32>,
    pub visible: Option<bool>,
    pub permission_id: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct MenuResponse {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    pub path: String,
    pub component: Option<String>,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub visible: bool,
    pub status: String,
    pub permission_id: Option<String>,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
    pub children: Vec<MenuResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MenuListResponse {
    pub items: Vec<MenuResponse>,
    pub total: u64,
}
```

- [ ] **Step 3: dto/user.rs 加 ToSchema**

```rust
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateUserRequest {
    #[validate(length(min = 1, max = 50))]
    pub username: String,
    #[validate(length(min = 8, max = 128))]
    pub password: String,
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(min = 11, max = 11))]
    pub phone: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub nickname: String,
    pub avatar: Option<String>,
    pub role_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateUserRequest {
    #[validate(length(min = 1, max = 100))]
    pub nickname: Option<String>,
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(min = 11, max = 11))]
    pub phone: Option<String>,
    pub avatar: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct UserQuery {
    pub keyword: Option<String>,
    pub status: Option<String>,
    pub org_unit_id: Option<String>,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub nickname: String,
    pub avatar: Option<String>,
    pub status: String,
    pub email_verified: bool,
    pub phone_verified: bool,
    pub last_login_at: Option<DateTime<chrono::Utc>>,
    pub last_login_ip: Option<String>,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserListResponse {
    pub items: Vec<UserResponse>,
    pub total: u64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchImportUsersRequest {
    pub users: Vec<CreateUserRequest>,
}
```

- [ ] **Step 4: 编译校验**

Run: `cargo build -p iam-service`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add services/iam-service/src/dto
git commit -m "feat(iam-service): add ToSchema/IntoParams derives on DTOs"
```

---

## Task 6: iam-service 路由加 #[utoipa::path]

**Files:**
- Modify: `services/iam-service/src/routes/auth.rs`
- Modify: `services/iam-service/src/routes/menus.rs`
- Modify: `services/iam-service/src/routes/users.rs`

- [ ] **Step 1: routes/auth.rs 五个核心 handler 加注解**

`login`：
```rust
#[utoipa::path(
    post,
    path = "/iam/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, body = LoginResponse),
        (status = 401, description = "invalid credentials"),
    ),
    tag = "auth"
)]
async fn login(
```

`refresh`：
```rust
#[utoipa::path(
    post,
    path = "/iam/auth/refresh",
    request_body = RefreshRequest,
    responses(
        (status = 200, body = LoginResponse),
        (status = 401, description = "token expired"),
    ),
    tag = "auth"
)]
async fn refresh(
```

`logout`：
```rust
#[utoipa::path(
    post,
    path = "/iam/auth/logout",
    responses((status = 204, description = "logged out")),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
async fn logout(
```

`change_password`：
```rust
#[utoipa::path(
    post,
    path = "/iam/auth/password",
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "password changed"),
        (status = 401, description = "invalid credentials"),
    ),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
async fn change_password(
```

`list_sessions`：
```rust
#[utoipa::path(
    get,
    path = "/iam/auth/sessions",
    responses((status = 200, body = [crate::dto::SessionInfo])),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
async fn list_sessions(
```

- [ ] **Step 2: routes/menus.rs 五个 handler 加注解**

```rust
#[utoipa::path(
    get,
    path = "/iam/menus",
    responses((status = 200, body = MenuListResponse)),
    security(("bearer_auth" = [])),
    tag = "menus"
)]
async fn list_menus(

#[utoipa::path(
    post,
    path = "/iam/menus",
    request_body = CreateMenuRequest,
    responses(
        (status = 200, body = MenuResponse),
        (status = 409, description = "duplicate menu"),
    ),
    security(("bearer_auth" = [])),
    tag = "menus"
)]
async fn create_menu(

#[utoipa::path(
    get,
    path = "/iam/menus/{menu_id}",
    params(("menu_id" = String, Path,)),
    responses(
        (status = 200, body = MenuResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "menus"
)]
async fn get_menu(

#[utoipa::path(
    put,
    path = "/iam/menus/{menu_id}",
    params(("menu_id" = String, Path,)),
    request_body = UpdateMenuRequest,
    responses(
        (status = 200, body = MenuResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "menus"
)]
async fn update_menu(

#[utoipa::path(
    delete,
    path = "/iam/menus/{menu_id}",
    params(("menu_id" = String, Path,)),
    responses(
        (status = 204, description = "deleted"),
        (status = 404, description = "not found"),
        (status = 409, description = "conflict"),
    ),
    security(("bearer_auth" = [])),
    tag = "menus"
)]
async fn delete_menu(
```

- [ ] **Step 3: routes/users.rs 五个 handler 加注解**

```rust
#[utoipa::path(
    get,
    path = "/iam/users",
    params(UserQuery),
    responses((status = 200, body = UserListResponse)),
    security(("bearer_auth" = [])),
    tag = "users"
)]
async fn list_users(

#[utoipa::path(
    post,
    path = "/iam/users",
    request_body = CreateUserRequest,
    responses(
        (status = 200, body = UserResponse),
        (status = 409, description = "duplicate user"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
async fn create_user(

#[utoipa::path(
    get,
    path = "/iam/users/{user_id}",
    params(("user_id" = String, Path,)),
    responses(
        (status = 200, body = UserResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
async fn get_user(

#[utoipa::path(
    put,
    path = "/iam/users/{user_id}",
    params(("user_id" = String, Path,)),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, body = UserResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
async fn update_user(

#[utoipa::path(
    delete,
    path = "/iam/users/{user_id}",
    params(("user_id" = String, Path,)),
    responses(
        (status = 204, description = "deleted"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
async fn delete_user(
```

- [ ] **Step 4: 把 auth/menus/users 的私有函数改成 `pub`（utoipa 派生需引用）**

将 `async fn login(...)` 改为 `pub async fn login(...)`，对所有 12 个被注解的 handler 同样处理（auth.rs 5 个、menus.rs 5 个、users.rs 5 个）。

- [ ] **Step 5: 编译校验**

Run: `cargo build -p iam-service`
Expected: 编译通过

- [ ] **Step 6: Commit**

```bash
git add services/iam-service/src/routes
git commit -m "feat(iam-service): annotate core routes with utoipa::path"
```

---

## Task 7: iam-service ApiDoc 与 main 集成

**Files:**
- Create: `services/iam-service/src/openapi.rs`
- Modify: `services/iam-service/src/lib.rs`
- Modify: `services/iam-service/src/main.rs`
- Modify: `services/iam-service/Cargo.toml` (确认 utoipa-swagger-ui 存在)

- [ ] **Step 1: 创建 openapi.rs**

写入 `services/iam-service/src/openapi.rs`：

```rust
use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::dto;
use crate::routes::{auth, menus, users};

pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi
            .components
            .get_or_insert_with(utoipa::openapi::Components::new);
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    info(title = "iam-service", description = "Identity, role, menu and user management"),
    paths(
        auth::login,
        auth::refresh,
        auth::logout,
        auth::change_password,
        auth::list_sessions,
        menus::list_menus,
        menus::create_menu,
        menus::get_menu,
        menus::update_menu,
        menus::delete_menu,
        users::list_users,
        users::create_user,
        users::get_user,
        users::update_user,
        users::delete_user,
    ),
    components(schemas(
        dto::LoginRequest,
        dto::LoginResponse,
        dto::RefreshRequest,
        dto::RefreshResponse,
        dto::ChangePasswordRequest,
        dto::UserInfo,
        dto::SessionInfo,
        dto::CreateMenuRequest,
        dto::UpdateMenuRequest,
        dto::MenuResponse,
        dto::MenuListResponse,
        dto::CreateUserRequest,
        dto::UpdateUserRequest,
        dto::UserResponse,
        dto::UserListResponse,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Authentication"),
        (name = "menus", description = "Menu management"),
        (name = "users", description = "User management"),
    )
)]
pub struct ApiDoc;
```

- [ ] **Step 2: lib.rs 导出 openapi**

修改 `services/iam-service/src/lib.rs`：

```rust
pub mod auth;
pub mod config;
pub mod db;
pub mod dto;
pub mod error;
pub mod models;
pub mod openapi;
pub mod routes;
pub mod service;
pub mod state;

pub use config::{load_config, Config};
```

- [ ] **Step 3: 修改 main.rs 挂 SwaggerUi**

修改 `services/iam-service/src/main.rs`：

文件头加：
```rust
use iam_service::openapi::ApiDoc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
```

替换 `let app = routes::build_router(state);` 周围逻辑为：

```rust
    let swagger_enabled = std::env::var("SWAGGER_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);

    let mut app = routes::build_router(state);
    if swagger_enabled {
        app = app.merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", ApiDoc::openapi()),
        );
        info!("swagger ui enabled at /swagger-ui");
    }
```

- [ ] **Step 4: 验证 Cargo.toml 已含 utoipa-swagger-ui**

Run:
```powershell
Select-String -Path services\iam-service\Cargo.toml -Pattern "utoipa-swagger-ui"
```
Expected: 输出包含 `utoipa-swagger-ui = "4"`。若缺失，添加该行。

- [ ] **Step 5: 构建校验**

Run: `cargo build -p iam-service`
Expected: 编译通过

- [ ] **Step 6: Commit**

```bash
git add services/iam-service/src/openapi.rs services/iam-service/src/lib.rs services/iam-service/src/main.rs
git commit -m "feat(iam-service): mount swagger-ui under SWAGGER_ENABLED flag"
```

---

## Task 8: iam-service 集成测试

**Files:**
- Create: `services/iam-service/tests/swagger_test.rs`

- [ ] **Step 1: 写测试**

写入 `services/iam-service/tests/swagger_test.rs`：

```rust
use iam_service::openapi::ApiDoc;
use utoipa::OpenApi;

#[test]
fn openapi_contains_core_paths() {
    let doc = ApiDoc::openapi();
    let json = serde_json::to_value(&doc).expect("serialize openapi");
    let paths = json
        .get("paths")
        .and_then(|v| v.as_object())
        .expect("paths object");

    assert!(paths.contains_key("/iam/auth/login"));
    assert!(paths.contains_key("/iam/auth/refresh"));
    assert!(paths.contains_key("/iam/menus"));
    assert!(paths.contains_key("/iam/menus/{menu_id}"));
    assert!(paths.contains_key("/iam/users"));
    assert!(paths.contains_key("/iam/users/{user_id}"));
}

#[test]
fn openapi_has_bearer_security_scheme() {
    let doc = ApiDoc::openapi();
    let json = serde_json::to_value(&doc).expect("serialize openapi");
    let scheme = json
        .pointer("/components/securitySchemes/bearer_auth")
        .expect("bearer_auth scheme present");
    assert_eq!(scheme.get("type").and_then(|v| v.as_str()), Some("http"));
    assert_eq!(scheme.get("scheme").and_then(|v| v.as_str()), Some("bearer"));
    assert_eq!(scheme.get("bearerFormat").and_then(|v| v.as_str()), Some("JWT"));
}
```

注：`iam_service` crate 名需与 `Cargo.toml` 中 `[package].name` 保持一致；若实际为 `iam-service`，则 use 仍为 `iam_service`（cargo 自动把 `-` 转 `_`）。

- [ ] **Step 2: 运行测试**

Run: `cargo test -p iam-service --test swagger_test`
Expected: 两个测试通过

- [ ] **Step 3: Commit**

```bash
git add services/iam-service/tests/swagger_test.rs
git commit -m "test(iam-service): verify openapi paths and bearer scheme"
```

---

## Task 9: 全量 workspace 校验

- [ ] **Step 1: workspace 整体编译**

Run: `cargo build --workspace`
Expected: 全部编译通过、无新增 warning。

- [ ] **Step 2: 仅测两服务的 swagger 测试**

Run:
```bash
cargo test -p token-service --test swagger_test
cargo test -p iam-service --test swagger_test
```
Expected: 全部 PASS。

- [ ] **Step 3: 推送分支（仅在用户授权时执行）**

⚠️ 默认不推送。仅当用户明确说 "push" 时执行：
```bash
git push -u origin feat/swagger-ui
```

---

## Self-Review

- 范围覆盖：spec 第 4 节列出的所有文件均在 Task 1–8 中处理。
- 类型一致：`AccessRequest/Response`、`MenuResponse`、`UserListResponse` 等 schema 名称在路由注解、`ApiDoc::components(schemas(...))` 与测试中保持一致。
- 占位符扫描：无 TBD/TODO/「类似前面」类引用。
- 命令：windows powershell 下 `Select-String` 替代 `grep`，符合工作区规则。
- 沙箱限制：`cargo build/test` 必须在真实 PowerShell（vcvarsall x64）执行，按项目规则提示用户在外部终端运行。

---

**Plan complete and saved to `docs/superpowers/plans/2026-06-16-swagger-ui.md`. Two execution options:**

1. **Subagent-Driven (recommended)** — 每个 Task 派一个新 subagent，逐步审阅
2. **Inline Execution** — 当前会话按 checkpoint 批量执行

**Which approach?**
