# token-service-sdk

Rust client SDK for the KudiCoder `token-service`. Endpoint paths and
request/response shapes come from `services/token-service/openapi.yaml`.

## Install

```toml
[dependencies]
token-service-sdk = { path = "../token-service-sdk" }
tokio = { version = "1", features = ["full"] }
```

## Usage

```rust
use std::sync::{Arc, Mutex};
use token_service_sdk::{TokenClient, AccessRequest};

let token = Arc::new(Mutex::new(Some("user.jwt".to_string())));
let client = TokenClient::new("https://gw.kudi.repos", move || {
    token.lock().unwrap().clone()
});

let resp = client.access(&AccessRequest {
    grant_type: "authorization_code".into(),
    code: "abc".into(),
    client_id: "client-1".into(),
    client_secret: "secret-1".into(),
}).await?;

println!("new access token: {}", resp.access_token);
```

## Methods

| Method | HTTP | Path |
|---|---|---|
| `access(&AccessRequest)` | POST | `/tokens/access` |
| `refresh(&RefreshRequest)` | POST | `/tokens/refresh` |
| `revoke(&RevokeRequest)` | POST | `/tokens/revoke` |
| `list_api_keys(&ListApiKeysQuery)` | GET | `/api-keys` |
| `create_api_key(&CreateApiKeyRequest)` | POST | `/api-keys` |

Not wrapped: `/tokens/user-jwt`, `/admin/oauth-clients/*` admin endpoints
(extend following the same pattern when needed).

## Errors

All methods return `Result<T, kudi_http_client::SdkError>` (re-exported as
`token_service_sdk::SdkError`). Five variants per the multi-lang SDK
design spec §9: `Auth` / `Validation` / `RateLimit` / `Server` /
`Network`.

## Notes

- `ApiKeyCreated::api_key_secret` is returned **exactly once** by
  `token-service`. The SDK does not cache it; persist it on receipt.
- `TokenClient` does not own token refresh; if you need a background
  refresher, do it in the caller.