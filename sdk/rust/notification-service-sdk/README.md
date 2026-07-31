# notification-service-sdk

Rust client SDK for the KudiCoder `notification-service`. Endpoint paths
and request/response shapes from
`services/notification-service/openapi.yaml`.

## Install

```toml
[dependencies]
notification-service-sdk = { path = "../notification-service-sdk" }
tokio = { version = "1", features = ["full"] }
```

## Usage

```rust
use std::sync::{Arc, Mutex};
use digua_notification_sdk::{NotificationClient, ListAnnouncementsQuery};

let token = Arc::new(Mutex::new(Some("user.jwt".to_string())));
let client = NotificationClient::new("https://gw.kudi.repos", move || {
    token.lock().unwrap().clone()
});

let page = client
    .list_announcements(&ListAnnouncementsQuery::new().page(0).size(20))
    .await?;
for a in page.items {
    println!("{}: {}", a.id, a.title);
}
```

## Methods

| Method | HTTP | Path |
|---|---|---|
| `list_announcements(&ListAnnouncementsQuery)` | GET | `/notification/announcements` |
| `get_announcement(id)` | GET | `/notification/announcements/{id}` |
| `create_announcement(&CreateAnnouncement)` | POST | `/notification/announcements` |
| `publish(id)` | POST | `/notification/announcements/{id}/publish` |
| `revoke(id)` | POST | `/notification/announcements/{id}/revoke` |
| `unread_count()` | GET | `/notification/unread-count` |

**Not wrapped**: `POST /notification/announcements/{id}/read` and the
`/notification/ws` WebSocket. Extend the SDK if you need them.

## Errors

All methods return `Result<T, digua_http_client::SdkError>` (re-exported
as `digua_notification_sdk::SdkError`). Five variants per the
multi-lang SDK design spec §9: `Auth` / `Validation` / `RateLimit` /
`Server` / `Network`.