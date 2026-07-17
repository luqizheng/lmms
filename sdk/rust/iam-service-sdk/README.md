# iam-service-sdk

Rust client SDK for the KudiCoder `iam-service`.

Endpoint paths and request/response shapes are taken from
`services/iam-service/openapi.yaml` (utoipa-generated single source of
truth). Endpoints that are not currently exposed in the OpenAPI spec are
not wrapped here; extend the SDK following the existing patterns if you
need them.

## Install

```toml
[dependencies]
iam-service-sdk = { path = "../iam-service-sdk" }
tokio = { version = "1", features = ["full"] }
```

## Usage — password login

```rust
use iam_service_sdk::IamClient;

let (client, login) = IamClient::login(
    "https://gw.kudi.repos",
    "alice",
    "secret",
).await?;

println!("hello {}", login.user.nickname);

let page = client
    .list_users(&iam_service_sdk::ListUsersQuery::new().page(0).size(20))
    .await?;
```

## Usage — bearer passthrough

```rust
use std::sync::{Arc, Mutex};
use iam_service_sdk::IamClient;

let token = Arc::new(Mutex::new(Some("user.jwt".to_string())));
let client = IamClient::new("https://gw.kudi.repos", move || {
    token.lock().unwrap().clone()
});

client.logout().await?;
```

## Methods

### Auth

| Method | HTTP | Path |
|---|---|---|
| `IamClient::login(base_url, user, pwd)` | POST | `/iam/auth/login` (constructor) |
| `refresh(refresh_token)` | POST | `/iam/auth/refresh` |
| `logout()` | POST | `/iam/auth/logout` |
| `change_password(old, new)` | POST | `/iam/auth/password` |

### Users

| Method | HTTP | Path |
|---|---|---|
| `list_users(&ListUsersQuery)` | GET | `/iam/users` |
| `get_user(user_id)` | GET | `/iam/users/{user_id}` |
| `create_user(&CreateUserRequest)` | POST | `/iam/users` |
| `update_user(user_id, &UpdateUserRequest)` | POST | `/iam/users/{user_id}` |

DELETE on `/iam/users/{user_id}` is not wrapped — `kudi-http-client`'s
`HttpTransport` trait currently only exposes GET / POST. Add a `delete`
method to `KudiWebClient` and a `delete_json` to `HttpTransport` first.

### Roles

| Method | HTTP | Path |
|---|---|---|
| `list_roles(&ListRolesQuery)` | GET | `/iam/roles` |
| `get_role(role_id)` | GET | `/iam/roles/{role_id}` |
| `create_role(&CreateRoleRequest)` | POST | `/iam/roles` |
| `update_role(role_id, &UpdateRoleRequest)` | POST | `/iam/roles/{role_id}` |

### Permissions

| Method | HTTP | Path |
|---|---|---|
| `list_permissions()` | GET | `/iam/permissions` |

## Errors

All methods return `Result<T, kudi_http_client::SdkError>` (re-exported as
`iam_service_sdk::SdkError`). Five variants per
`docs/superpowers/specs/2026-06-08-multi-lang-sdk-design.md` §9:
`Auth` / `Validation` / `RateLimit` / `Server` / `Network`.