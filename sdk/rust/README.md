# KudiCoder Rust SDK Suite

Typed Rust clients for the KudiCoder microservices platform. The shared
HTTP + token-cache layer is `kudi-http-client`; one crate per public
service wraps that layer with service-specific typed methods and models.

Endpoint paths and request/response shapes are sourced from each
service's auto-generated `services/<svc>/openapi.yaml` — Java SDK
paths have diverged in places, so do not copy them blindly.

## Crates

| Crate | Purpose | Endpoints wrapped |
|---|---|---|
| [`kudi-http-client`](kudi-http-client) | Shared transport + token cache | (not a service client — used by the SDKs below) |
| [`iam-service-sdk`](iam-service-sdk) | `iam-service` (auth + user/role/permission/tenant management) | login/refresh/logout/password + users/roles/permissions CRUD |
| [`token-service-sdk`](token-service-sdk) | `token-service` (OAuth + API key) | access/refresh/revoke + api-keys CRUD |
| [`audit-service-sdk`](audit-service-sdk) | `audit-service` (operation & login logs + retention) | query login/operation logs + retention policy CRUD |
| [`file-service-sdk`](file-service-sdk) | `file-service` (presigned upload/download + metadata) | presign + list + info (binary transfer done via presign URL) |
| [`notification-service-sdk`](notification-service-sdk) | `notification-service` (announcements + unread count) | announcement CRUD + publish/revoke + unread count |
| [`config-service-sdk`](config-service-sdk) | `config-service` (namespaces + items + watch) | namespace + item CRUD (SSE watch left for SSE clients) |

All six service SDKs depend on `kudi-http-client` (workspace path).
Bring them in with `path = "..."` while consuming them locally; publishing
to an internal `crates.io` mirror is a follow-up.

## Build and test

```bash
# Just the SDK crates
cargo build -p kudi-http-client \
  -p iam-service-sdk \
  -p token-service-sdk \
  -p audit-service-sdk \
  -p file-service-sdk \
  -p notification-service-sdk \
  -p config-service-sdk

cargo test -p kudi-http-client \
  -p iam-service-sdk \
  -p token-service-sdk \
  -p audit-service-sdk \
  -p file-service-sdk \
  -p notification-service-sdk \
  -p config-service-sdk
```

Use `CARGO_TARGET_DIR=/tmp/sdk-target` if the repo-root `target/` is
root-owned from a docker volume (see root `AGENTS.md`).

## Errors

Every method returns `Result<T, digua_http_client::SdkError>`
(re-exported as `<service>_sdk::SdkError`). The five variants per
[`docs/superpowers/specs/2026-06-08-multi-lang-sdk-design.md` §9](../../docs/superpowers/specs/2026-06-08-multi-lang-sdk-design.md):

- `Auth { status, message, details }` — HTTP 401/403
- `Validation { status, message, details }` — HTTP 400/422
- `RateLimit { message, details }` — HTTP 429
- `Server { status, message, details }` — HTTP 5xx
- `Network(String)` — connection / decode / serialization

## Authentication

Most service SDKs take a `Fn() -> Option<String>` bearer supplier so
the caller owns the user JWT (typically the one issued by `iam-service`
login). For service-to-service flows where you have a client
credential pair, `kudi-http-client` also supports `client_credentials`
with automatic token caching and refresh (`force_refresh()` to
invalidate after a 401).

## Extending the SDKs

Each service SDK follows the same pattern — read the corresponding
`services/<svc>/openapi.yaml`, add a `serde::{Serialize, Deserialize}`
struct to `src/model.rs`, add a method to `src/client.rs` that
composes the path, calls `http.get/post` with the supplier's bearer,
maps non-2xx responses to `SdkError`, and round-trips the body
through serde. Add a wiremock test in `tests/client_test.rs` covering
at least request shape, success parsing, and 401 → `SdkError::Auth`.

## Out of scope

- **Service internal traffic** — Rust services don't use this SDK; they
  use `libs/digua_common-http`'s `ResilientClient` + `ServiceJwtCache` for
  internal m2m (different trust domain, different auth model).
- **Java SDK** — unchanged, lives in `sdk/java/`.
- **Multipart upload / binary download in file-service-sdk** —
  `kudi-http-client`'s `HttpResponse` is text-only. Presign and use any
  HTTP client directly against the storage URL.
- **WebSocket / SSE** — out of scope for `kudi-http-client`. Use a
  dedicated streaming client (`tokio-tungstenite`, `eventsource-client`,
  etc.) against the public WebSocket/SSE URL.
- **CI gate** — no new required gate. CI `test` stage does not run
  these; integrate via the existing docker rust-test workflow if you
  need CI coverage.