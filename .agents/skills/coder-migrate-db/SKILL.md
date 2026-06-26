---
name: coder-migrate-db
description: |
  Scaffold a database migration with dual MySQL/SQLite dialects, up/down conventions,
  and proper timestamp naming. Use when the user says "add a migration", "create a
  table", "add a column", or any database schema change intent.
---

# coder-migrate-db Skill

Scaffold a SQL migration for an existing Rust microservice.

## Service Scope

Only these services have migration infrastructure — ask the user to pick one:
- `audit-service`
- `config-service`
- `file-service`
- `iam-service` (uses `migrations/` directly, not mysql/sqlite subdirs)
- `notification-service` (uses `YYYY_MM_DD_*.sql` naming)
- `prue_backend` (uses `migrations/{mysql,sqlite}/`)
- `token-service` (uses `migrations/` directly`)

## Table Naming Convention

**All tables MUST be prefixed with the service prefix** to avoid collisions in the shared `superpowers` MySQL database. See `contracts/db-conventions.md` for the full convention.

| Service | Prefix | Example |
|---------|--------|---------|
| `iam-service` | `iam_` | `iam_users`, `iam_roles` |
| `audit-service` | `audit_` | `audit_operation_logs`, `audit_login_logs` |
| `token-service` | `token_` | `token_sessions`, `token_revocations` |
| `notification-service` | `ntf_` | `ntf_announcements` |
| `config-service` | `cfg_` | `cfg_namespaces`, `cfg_items` |
| `file-service` | `file_` | `file_files`, `file_presign_urls` |
| `prue_backend` | `prue_` | `prue_menu_configs` |

**Index naming**: `idx_<table>_<col1>_<col2>` (use the full prefixed table name). UKs: `uk_<table>_<col>`. FKs: `fk_<table>_<col>`.

A CI gate enforces this: `node scripts/check-db-naming.js` fails if any table reference in `services/*/migrations/**/*.sql` or `services/*/src/**/*.rs` (`sqlx::query*` calls) lacks its service prefix. Run before committing migrations.

## Gathering Migration Info

Ask the user for:

1. **Service name** (e.g. `prue_backend`)
2. **Migration description** (snake_case, e.g. `add_user_roles`, `extend_session_timeout`) — used in filename
3. **Table name with prefix** (e.g. `prue_menu_configs`, `iam_user_sessions`) — see table naming convention above
4. **Column definitions** — each as: `name type [NULL | NOT NULL] [DEFAULT value] [PRIMARY KEY | UNIQUE]`
5. **Indexes** (optional): `INDEX index_name (col1, col2)` or `UNIQUE INDEX unique_name (col1)`
6. **Foreign keys** (optional): `FOREIGN KEY (col) REFERENCES other_table(col)` — note that the referenced table must also be prefixed

## Timestamp Convention

Use UTC timestamp for filename: `V{YYYYMMDDHHMMSS}__{description}.sql`

Generate with: `date -u +%Y%m%d%H%M%S` locally, or use `20250625120000` as placeholder and note the developer should verify the timestamp is unique and in the future relative to existing migrations.

## MySQL Dialect Rules

- SERIAL → `BIGINT NOT NULL AUTO_INCREMENT`
- TEXT → `TEXT`
- VARCHAR(n) → `VARCHAR(n)`
- BOOLEAN → `TINYINT(1) NOT NULL DEFAULT 0`
- TIMESTAMPS → `TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP`
- Soft deletes → `deleted_at TIMESTAMP NULL`
- JSON → `JSON`
- AUTO_INCREMENT primary key: `id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY`

## SQLite Dialect Rules

- SERIAL → `INTEGER NOT NULL`
- TEXT → `TEXT`
- VARCHAR(n) → `TEXT` (SQLite has no VARCHAR, use TEXT with application-level validation)
- BOOLEAN → `INTEGER NOT NULL DEFAULT 0`
- TIMESTAMPS → `DATETIME NOT NULL DEFAULT (datetime('now'))`
- Soft deletes → `deleted_at DATETIME NULL`
- JSON → `TEXT` (store as JSON string)
- AUTOINCREMENT: `id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT`

## File Scaffolding

### Standard services (audit/config/file/prue_backend)

**`services/{svc}/migrations/mysql/V{timestamp}__{description}.sql`**:
```sql
-- up
CREATE TABLE {table} (
    id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    {columns_with_commas},
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP NULL,
    {indexes}
);

-- down
DROP TABLE {table};
```

**`services/{svc}/migrations/sqlite/V{timestamp}__{description}.sql`**:
```sql
-- up
CREATE TABLE {table} (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    {columns_with_commas_sqlite},
    created_at DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at DATETIME NOT NULL,
    deleted_at DATETIME NULL,
    {indexes}
);

-- down
DROP TABLE {table};
```

### iam-service and token-service (flat migrations/ directory)

**`services/{svc}/migrations/V{timestamp}__{description}.sql`**:
```sql
-- up
CREATE TABLE {table} (
    id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    {columns_with_commas},
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP NULL,
    {indexes}
);

-- down
DROP TABLE {table};
```

Note: These services only use MySQL; no SQLite version needed.

### notification-service (YYYY_MM_DD_ naming)

**`services/{svc}/migrations/{YYYY_MM_DD}_{description}.sql`**:
Same structure as above but filename format matches existing convention.

## Column Type Mapping

When the user says a type, map it:

| User says | MySQL | SQLite |
|-----------|-------|--------|
| `string` | VARCHAR(255) | TEXT |\
| `text` | TEXT | TEXT |
| `boolean` | TINYINT(1) NOT NULL DEFAULT 0 | INTEGER NOT NULL DEFAULT 0 |
| `integer`, `id` | BIGINT NOT NULL AUTO_INCREMENT | INTEGER NOT NULL |
| `bigint` | BIGINT NOT NULL | INTEGER NOT NULL |
| `datetime` | TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP | DATETIME NOT NULL DEFAULT (datetime('now')) |
| `uuid` | VARCHAR(36) NOT NULL | TEXT NOT NULL |
| `json` | JSON NOT NULL | TEXT NOT NULL |

## Index Conventions

- Default index name: `idx_{table}_{column}` (e.g. `idx_menu_configs_active`)
- Composite index: `idx_{table}_{col1}_{col2}`
- UNIQUE index: `uk_{table}_{column}`

## Post-Migration Steps

1. Run `cargo check -p {svc}` inside docker to verify the migration is syntactically valid (the SQL won't execute during check, but sqlx compile-time checking will catch type mismatches)
2. If the service uses sqlx macros (e.g. `sqlx::query!`), update the corresponding model struct in `src/models/` to match the new column
3. Add the migration to version control immediately

## Validation Checklist

- [ ] Both MySQL and SQLite files created (unless flat migrations dir)
- [ ] Filename timestamp is unique and in the future
- [ ] `id` column is present as auto-increment primary key
- [ ] `deleted_at` soft-delete column is present
- [ ] `created_at` and `updated_at` timestamps are present
- [ ] Down migration correctly drops the table
- [ ] No MySQL-specific syntax in SQLite file (no `AUTO_INCREMENT`, no `VARCHAR`)
- [ ] Index names are unique across the table
