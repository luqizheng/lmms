# kudi-http-client (Rust)

Generic HTTP client for inter-service calls. Two modes:

- **Service-to-service** — `KudiWebClient::builder().credential(client_id, secret_key)`; subsequent `.get()` / `.post()` automatically fetch and cache a `client_credentials` token.
- **User Bearer passthrough** — call `.get_with_bearer(url, &user_jwt)`.

Token cache is keyed by `SHA256(client_id | secret_key)` and shared across `KudiWebClient` instances via a static `DashMap`. Default `expires_in - 60s` pre-refresh.

## Install

```toml
[dependencies]
kudi-http-client = { path = "../kudi-http-client" }
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

## Usage

```rust
use kudi_http_client::KudiWebClient;

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

`kudi_http_client::KudiHttpError`:

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
| `user_agent` | `kudi-http-client/<version>` |

The first call to a transport installs the rustls `aws_lc_rs` crypto provider (idempotent).
