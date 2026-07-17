# config-service-sdk

Rust client SDK for the KudiCoder `config-service`. Endpoint paths and
request/response shapes from `services/config-service/openapi.yaml`.

## Install

```toml
[dependencies]
config-service-sdk = { path = "../config-service-sdk" }
tokio = { version = "1", features = ["full"] }
```

## Usage

```rust
use std::sync::{Arc, Mutex};
use config_service_sdk::{ConfigClient, ListNamespacesQuery};

let token = Arc::new(Mutex::new(Some("user.jwt".to_string())));
let client = ConfigClient::new("https://gw.kudi.repos", move || {
    token.lock().unwrap().clone()
});

let page = client
    .list_namespaces(&ListNamespacesQuery::new().app("iam-service").env("prod"))
    .await?;
```

## Methods

| Method | HTTP | Path |
|---|---|---|
| `list_namespaces(&ListNamespacesQuery)` | GET | `/config/namespace` |
| `create_namespace(&CreateNamespace)` | POST | `/config/namespace` |
| `list_items(&ListItemsQuery)` | GET | `/config/item` |
| `create_item(&CreateConfigItem)` | POST | `/config/item` |

**Not wrapped**: `GET/PUT /config/namespace/{id}`, `GET/PUT /config/item/{id}`,
`GET /config/watch` (SSE), `POST /config/export/{id}` /
`POST /config/import/{id}`. Extend the SDK following the same pattern.

## Errors

All methods return `Result<T, kudi_http_client::SdkError>` (re-exported
as `config_service_sdk::SdkError`). Five variants per the multi-lang SDK
design spec §9: `Auth` / `Validation` / `RateLimit` / `Server` /
`Network`.