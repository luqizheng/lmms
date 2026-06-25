---
name: coder-add-route
description: |
  Scaffold a new route handler in an existing Rust service with proper utoipa
  annotations, OpenAPI registration, and unit test stub. Use when the user says
  "add a route", "create an endpoint", "add a handler", or any equivalent intent.
---

# coder-add-route Skill

Scaffold a new route in an existing Rust microservice.

## Service Scope

Only these services are supported — ask the user to pick one if they don't specify:
- `audit-service`
- `config-service`
- `file-service`
- `prue_backend`
- `iam-service`
- `notification-service`
- `token-service`

## Gathering Route Info

Ask the user for (skip any the user already provided):

1. **Service name** (e.g. `audit-service`)
2. **Route path** (e.g. `/audit/feature` or `/pure/items`)
3. **HTTP method** (`GET`, `POST`, `PUT`, `DELETE`)
4. **Handler function name** (e.g. `create_feature`, `list_features`) — must be snake_case
5. **Request body DTO** (e.g. `CreateFeatureRequest`) — omit if no body
6. **Response DTO** (e.g. `FeatureDetail`, `serde_json::Value`) — use `serde_json::Value` for simple objects
7. **Query params struct name** (for GET only, e.g. `QueryFeatures`) — omit for POST/PUT/DELETE
8. **OpenAPI tag** (grouping label, e.g. `feature`) — defaults to first path segment

## File Scaffolding

### Step 1: Create `services/{svc}/src/routes/{feature}.rs`

```rust
use axum::{
    extract::{Query, State},
    Json, Router,
};
use crate::error::AppResult;
use crate::routes::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/{feature}", axum::routing::{METHOD}({handler_name}))
        .with_state(state)
}

#[utoipa::path(
    {METHOD},
    path = "{full_external_path}",
    {request_body or params or nothing},
    responses(
        (status = 200, description = "...", body = {ResponseType}),
    ),
    tag = "{tag}"
)]
pub async fn {handler_name}(
    State(state): State<AppState>,
    {extractors}
) -> AppResult<Json<{ResponseType}>> {
    todo!("implement {handler_name}")
}
```

Rules:
- `{METHOD}` = `get`, `post`, `put`, `delete` (lowercase axum routing method)
- `{feature}` = last path segment (e.g. `items` from `/audit/items`)
- `{full_external_path}` = the full external path as registered in the router (e.g. `/audit/items`, `/pure/items`)
- `request_body = BodyType` goes inside `#[utoipa::path(...)]` for POST/PUT
- `params(QueryType)` goes inside `#[utoipa::path(...)]` for GET with query params
- Extractors: for GET with query params use `Query(query): Query<QueryType>`; for path params use `Path(id): Path<String>`
- Always include `State(state): State<AppState>` as first extractor
- If no body and no query params, no additional extractor needed

### Step 2: Update `services/{svc}/src/routes/mod.rs`

Add:
```rust
pub mod {feature};
```

In the `routes()` function, add the new nest:
```rust
.nest("/{prefix}", {feature}::routes(state.clone()))
```

Where `{prefix}` = first path segment (e.g. `audit`, `config`, `pure`).

### Step 3: Update `services/{svc}/src/openapi.rs`

Add the handler to the `paths(...)` list AND add any new DTO types to `components(schemas(...))`.

Example updated `#[openapi(...)]`:
```rust
#[derive(OpenApi)]
#[openapi(
    paths(
        // existing handlers...
        {feature}::{handler_name},
    ),
    components(schemas(
        // existing schemas...
        CreateFeatureRequest,
        FeatureDetail,
        QueryFeatures,
    )),
    tags(
        (name = "feature", description = "Feature endpoints"),
    )
)]
pub struct ApiDoc;
```

If the service has a `dto/` directory, put new DTO structs there. If not, put them at the top of the route file.

### Step 4: Create `services/{svc}/tests/{feature}_test.rs`

```rust
use crate::error::AppResult;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use tower::util::ServiceExt;

#[tokio::test]
async fn {handler_name}_returns_200() -> AppResult<()> {
    // TODO: set up AppState with mock services
    let app = Router::new()
        .route("{external_path}", axum::routing::{METHOD}(crate::routes::{feature}::{handler_name}))
        // .with_state(mock_state)
        ;

    let res = app
        .oneshot(Request::builder().method({METHOD_UPPER}).uri("{external_path}").body(Body::empty())?)
        .await?;

    assert_eq!(res.status(), StatusCode::OK);
    Ok(())
}
```

Use `tower::ServiceExt::oneshot` for testing routers. The test is a smoke test — the body assertion can be adjusted once the real implementation exists.

## OpenAPI Path Convention

- External path in `#[utoipa::path(path = "...")]` uses the full path as seen by API clients
- Internal route registration uses only the last segment with a `.nest()` at the prefix level
- Example: `/audit/items` → `nest("/audit", items::routes(state.clone()))` + `route("/items", get(list_items))`

## DTO Convention

- Create DTO structs in `services/{svc}/src/dto/` if the directory exists
- Name request DTOs `{Action}{Resource}Request` (e.g. `CreateItemRequest`)
- Name response DTOs `{Resource}Detail` or `{Resource}Summary`
- Name query DTOs `Query{Resource}` with `#[derive(utoipa::IntoParams)]`
- Derive `serde::Deserialize` and `serde::Serialize` on all DTOs
- Derive `utoipa::ToSchema` on response DTOs only

## Validation

After creating files, verify:
1. The route file compiles: `cargo check -p {svc}` inside docker
2. The openapi.rs has no duplicate paths in the `paths(...)` list
3. If a new DTO was added, it appears in `components(schemas(...))` in openapi.rs
4. The test file has no import errors
