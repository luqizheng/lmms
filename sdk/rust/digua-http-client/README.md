# digua_http_client (Rust)

Generic HTTP client for inter-service calls. Two modes:

- **Service-to-service** — `KudiWebClient::builder().credential(client_id, secret_key)`; subsequent `.get()` / `.post()` automatically fetch and cache a `client_credentials` token.
- **User Bearer passthrough** — call `.get_with_bearer(url, &user_jwt)`.

Token cache is keyed by `SHA256(client_id | secret_key)` and shared across `KudiWebClient` instances via a static `DashMap`. Default `expires_in - 60s` pre-refresh.

## Install

```toml
[dependencies]
digua_http_client = { path = "../digua_http_client" }
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

## Usage

```rust
use digua_http_client::KudiWebClient;

let client = KudiWebClient::builder()
    .credential("contract-svc", "secret-111")
    .build()?;

// service-to-service
let resp = client.get("http://user-svc/api/users/123", None).await?;
println!("{} {}", resp.status, resp.body);

// user Bearer passthrough
client.get_with_bearer("http://user-svc/api/users/me", &user_jwt).await?;

// POST JSON
client.post("http://user-svc/api/users", r#"{"name":"x"}"#, None).await?;
```

## Errors

`digua_http_client::KudiHttpError`:

- `Auth { status, message, details }` — HTTP 401/403
- `Validation { status, message, details }` — HTTP 400/422
- `RateLimit { message, details }` — HTTP 429
- `Server { status, message, details }` — HTTP 5xx
- `Network(String)` — connection/timeout

## Options

`KudiHttpOptions`:

| Field | Default |
|---|---|
| `token_endpoint` | `http://token-svc/api/tokens/access` |
| `connect_timeout` | 5s |
| `read_timeout` | 15s |
| `retry_count` | 1 (5xx/network only) |
| `refresh_skew` | 60s |
| `user_agent` | `digua_http_client/<version>` |

The first call to a transport installs the rustls `aws_lc_rs` crypto provider (idempotent).

## 作为 SDK 共享底座

This crate is the shared transport + token-cache layer for the per-service SDKs
(`digua_iam_sdk`, `digua_token_sdk`, `digua_audit_sdk`,
`digua_file_sdk`, `digua_notification_sdk`, `digua_config_sdk`).
If you are calling the platform from your own application you can also depend
on this crate directly and build the URL yourself.

### Public API used by service SDKs

| Symbol | Purpose |
|---|---|
| `SdkError` (= `KudiHttpError`) | Canonical error type — `Auth` / `Validation` / `RateLimit` / `Server` / `Network`. Service SDKs re-export this so callers don't need to know the internal name. |
| `KudiHttpClient::builder().base_url(...)` / `with_base_url(...)` | Stash the api-gateway base URL so the service SDK can compose `/v1/<svc>/...` paths without callers passing the prefix on every request. |
| `KudiWebClient::base_url()` | Read back the stashed base URL. |
| `KudiWebClient::force_refresh()` | Invalidate the cached client_credentials token after observing a 401 so the next call refetches. |

### Example wiring inside a service SDK

```rust
use digua_http_client::{KudiWebClient, KudiHttpOptions, RequestContext, SdkError};

// 1. Service SDK caller: provide base URL once.
let http = KudiWebClient::builder()
    .credential("contract-svc", "secret-111")
    .base_url("https://gw.kudi.repos")   // → stashed on the client
    .build()?;

// 2. Inside the SDK, compose paths against the stashed base URL.
let path = format!("{}/v1/iam/auth/login", http.base_url().unwrap());
let resp = http.post(&path, body, None).await?;

// 3. On 401, force a token refresh and retry once.
if resp.status == 401 {
    http.force_refresh().await?;
    let resp = http.post(&path, body, None).await?;
}
```

If you need per-call default headers (e.g. `X-Tenant-Id`, `X-Trace-Id`), build
them into the path or merge them into a `reqwest::header::HeaderMap` and pass
it via `client.get_with_bearer(...)` style usage — `HttpTransport` is trait-
sealed to avoid leaking reqwest-specific concepts into this crate.
