# P2.3 Coder Skills Design

## Context

Phase 2 calls for落地 skills: `coder-add-route` and `coder-migrate-db`. These are project-specific skill files that the AI can invoke to scaffold standardized project artifacts, reducing boilerplate and enforcing conventions automatically.

## Skills Location

`.agents/skills/coder-add-route/SKILL.md`
`.agents/skills/coder-migrate-db/SKILL.md`

## coder-add-route

**Purpose**: Scaffold a new route handler in an existing Rust service with proper utoipa annotations, OpenAPI registration, and unit test stub.

### Input (asked to user)
1. Target service name (e.g. `audit-service`, `config-service`)
2. Route path (e.g. `/audit/feature`)
3. HTTP method (`GET`, `POST`, `PUT`, `DELETE`)
4. Handler function name (e.g. `create_foo`, `list_foos`)
5. Request DTO type (if any)
6. Response DTO type (if any)
7. Query parameters struct name (for GET, if any)
8. Tag name for OpenAPI grouping

### Output (created/modified files)

**`services/{svc}/src/routes/{feature}.rs`**:
```rust
use axum::{extract::State, Json, Router};
use crate::error::AppResult;
use crate::routes::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/{feature}", axum::routing::{METHOD}({handler}))
        .with_state(state)
}

#[utoipa::path(
    {METHOD},
    path = "{full_path}",
    {params or request_body or nothing},
    responses(
        (status = 200, description = "...", body = ResponseType),
    ),
    tag = "{tag}"
)]
pub async fn {handler}(
    State(state): State<AppState>,
    {extractors}
) -> AppResult<Json<ResponseType>> {
    todo!("implement {handler}")
}
```

**`services/{svc}/src/routes/mod.rs`**: adds `pub mod {feature}` and `nest(...)` call.

**`services/{svc}/src/openapi.rs`**: adds handler to `paths(...)` list and DTO to `components(schemas(...))`.

**`services/{svc}/tests/{feature}_test.rs`**: unit test scaffold with one passing smoke test.

### Conventions
- Route file name = first segment of path (e.g. `/audit/foo` → `routes/foo.rs`)
- Handler function uses snake_case
- utoipa `tag` matches the route file name
- Always use `AppResult<Json<T>>` return type
- State extracted via `State<AppState>` as first extractor
- DTOs live in `models/` module; if DTO doesn't exist, skill prompts user

## coder-migrate-db

**Purpose**: Scaffold a database migration with proper timestamp naming, dual MySQL/SQLite dialects, and up/down conventions.

### Input (asked to user)
1. Target service name
2. Migration description (snake_case, e.g. `add_user_roles`)
3. Table name
4. Column definitions: `name type [NULL | NOT NULL] [DEFAULT value] [PRIMARY KEY | UNIQUE]`
5. Indexes (optional): `INDEX idx_name (col1, col2)`
6. Foreign keys (optional)

### Output (created files)

**`services/{svc}/migrations/mysql/V{timestamp}__{description}.sql`**:
```sql
-- up
CREATE TABLE {table} (
    {columns},
    {indexes},
    {foreign_keys}
);

-- down
DROP TABLE {table};
```

**`services/{svc}/migrations/sqlite/V{timestamp}__{description}.sql`**: Same structure, SQLite-compatible types.

### Conventions
- Timestamp format: `V{timestamp}__` where timestamp = current UTC datetime formatted as `%Y%m%d%H%M%S`
- Always generate both MySQL and SQLite versions
- `AUTO_INCREMENT` for MySQL SERIAL/INTEGER; `AUTOINCREMENT` for SQLite
- Use `TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP` for MySQL; `DATETIME NOT NULL DEFAULT (datetime('now'))` for SQLite
- Always include `deleted_at` soft-delete column if not specified
- Always add `-- up` / `-- down` comment markers
