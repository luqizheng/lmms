# digua_file_sdk

Rust client SDK for the KudiCoder `file-service`. Endpoint paths and
request/response shapes from `services/file-service/openapi.yaml`.

## Install

```toml
[dependencies]
digua_file_sdk = { path = "../digua_file_sdk" }
tokio = { version = "1", features = ["full"] }
```

## Usage

```rust
use std::sync::{Arc, Mutex};
use digua_file_sdk::{FileClient, CreatePresignUrl, ListFilesQuery};

let token = Arc::new(Mutex::new(Some("user.jwt".to_string())));
let client = FileClient::new("https://gw.kudi.repos", move || {
    token.lock().unwrap().clone()
});

let presign = client.create_presign(&CreatePresignUrl {
    file_id: "f-1".into(),
    operation: "upload".into(),
    expires_in_seconds: Some(3600),
}).await?;
// PUT your file bytes to presign.url directly with any HTTP client.

let page = client.list_files(&ListFilesQuery::new().page(0).size(20)).await?;
```

## Methods

| Method | HTTP | Path |
|---|---|---|
| `create_presign(&CreatePresignUrl)` | POST | `/file/presign` |
| `list_files(&ListFilesQuery)` | GET | `/file/list` |
| `get_file_info(id)` | GET | `/file/{id}/info` |

**Not wrapped**: `POST /file/upload` (multipart) and `GET /file/{id}`
(download). Both are byte-heavy — presign a URL and use any HTTP client
directly. `digua_http_client`'s `HttpResponse` is text-only, so wrapping
binary transfer would require extending that crate first.

## Errors

All methods return `Result<T, digua_http_client::SdkError>` (re-exported
as `digua_file_sdk::SdkError`). Five variants per the multi-lang SDK
design spec §9: `Auth` / `Validation` / `RateLimit` / `Server` /
`Network`.