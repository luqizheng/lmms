# digua_audit_sdk

Rust client SDK for the KudiCoder `audit-service`.

Endpoint paths are taken from `services/audit-service/openapi.yaml` — the
single source of truth. The Java SDK's paths (`/v1/audit/events`,
`/v1/audit/logs`, `/v1/audit/event-types`) are **stale and do not exist
in the current service**; this SDK targets the routes that are actually
exposed.

## Install

```toml
[dependencies]
digua_audit_sdk = { path = "../digua_audit_sdk" }
tokio = { version = "1", features = ["full"] }
```

## Usage

```rust
use std::sync::{Arc, Mutex};
use digua_audit_sdk::{AuditClient, LoginLogQuery};

let token = Arc::new(Mutex::new(Some("user.jwt".to_string())));
let client = AuditClient::new("https://gw.kudi.repos", move || {
    token.lock().unwrap().clone()
});

let page = client
    .query_login_logs(&LoginLogQuery::new().username("alice").page(0).size(20))
    .await?;

for log in page.items {
    println!("{}: {} ({})", log.created_at, log.username, log.success);
}
```

## Methods

| Method | HTTP | Path |
|---|---|---|
| `query_login_logs(&LoginLogQuery)` | GET | `/audit/login` |
| `query_operation_logs(&OperationLogQuery)` | GET | `/audit/operation` |
| `trigger_cleanup()` | POST | `/audit/retention/cleanup` |
| `get_retention_policy()` | GET | `/audit/retention/policy` |
| `update_retention_policy(&RetentionPolicy)` | POST | `/audit/retention/policy` |

Not covered (extend the SDK on demand): `export_*_csv/json` endpoints under
`/audit/export/compliance/...`.

## Errors

All methods return `Result<T, digua_http_client::SdkError>` (re-exported as
`digua_audit_sdk::SdkError`):

- `Auth { status, message, details }` — HTTP 401/403
- `Validation { status, message, details }` — HTTP 400/422
- `RateLimit { message, details }` — HTTP 429
- `Server { status, message, details }` — HTTP 5xx
- `Network(String)` — connection / decode / serialization errors

Aligns with `docs/superpowers/specs/2026-06-08-multi-lang-sdk-design.md` §9.

## Notes on bearer passthrough

`audit-service` consumes a user JWT issued by `iam-service`; this SDK does
not obtain or refresh tokens itself. The caller supplies a closure
(`Fn() -> Option<String>`) so each request picks up the current bearer
(thread-safety is the caller's responsibility — wrap with `Arc<Mutex<_>>`
if it needs to mutate).