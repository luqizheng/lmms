---
name: coder-migrate-java-db
description: |
  Scaffold a Flyway database migration for an existing Java Spring Boot service.
  MySQL only (Java services do not require dual MySQL/SQLite dialects unlike
  Rust's prue_backend). Use when the user says "add a migration", "create a
  table", "add a column" for a Java service, or any database schema change intent.
---

# coder-migrate-java-db Skill

Scaffold a Flyway SQL migration for an existing Java Spring Boot microservice.

## Service Scope

Any Java service under `services/{name}/` that has a `{short}-app-infrastructure`
module with Flyway configured. The migration files live in:

```
services/{name}/{short}-app-infrastructure/src/main/resources/db/migration/
```

Flyway runs migrations automatically at application startup (configured in
`application.yml` via `spring.flyway.locations: classpath:db/migration`).

## Variable Conventions

| Placeholder | Meaning | Example |
|---|---|---|
| `{name}` | kebab-case service name | `billing-service` |
| `{short}` | short name (strip `-service`) | `billing` |
| `{prefix}` | DB table prefix (with trailing `_`) | `billing_` |
| `{table}` | full table name (prefix + resource) | `billing_invoices` |
| `{description}` | snake_case migration description | `add_invoice_items` |

## Table Naming Convention

**All tables MUST be prefixed with the service prefix** to avoid collisions in
the shared `superpowers` MySQL database. See `contracts/db-conventions.md`.

The prefix must be registered in `scripts/check-db-naming.js` whitelist (this
is done during `coder-new-java-service` scaffolding). If adding a table for a
new resource that uses a different prefix within the same service, verify the
prefix is whitelisted.

**Naming rules** (same as Rust services, see `contracts/db-conventions.md`):

| Object | Pattern | Example |
|---|---|---|
| Table | `{prefix}<resource_plural_snake>` | `billing_invoices` |
| Index | `idx_<table>_<cols>` | `idx_billing_invoices_status` |
| Unique key | `uk_<table>_<cols>` | `uk_billing_invoices_number` |
| Foreign key | `fk_<table>_<col>` | `fk_billing_items_invoice_id` |

CI gate: `node scripts/check-db-naming.js` scans `services/**/*.sql` and
validates table references match the service prefix.

## Gathering Migration Info

Ask the user for:

1. **Service name** (e.g. `billing-service`)
2. **Migration description** (snake_case, e.g. `add_invoices`, `extend_invoice_lines`)
3. **Operation type** (create table / alter table / add column / add index / data backfill)
4. **Table name with prefix** (e.g. `billing_invoices`)
5. **Column definitions** (for CREATE TABLE): each as `name type [constraints]`
6. **Index definitions** (optional)
7. **Foreign keys** (optional) - referenced table must also be prefixed

## Version Number Convention

Flyway uses `V{N}__{description}.sql` where `{N}` is a sequential integer or
timestamp. Check existing migrations in the `db/migration/` directory and use
`V{next_number}__{description}.sql`.

```
V1__init.sql              <- initial schema (created by coder-new-java-service)
V2__add_invoices.sql      <- next migration
V3__extend_invoice_lines.sql
```

If the service uses timestamp-based versioning (e.g. `V20260714_120000__...`),
follow the existing convention in that service's migration directory.

**Never edit an existing migration file.** Flyway checksums each file and will
refuse to start if a checksum mismatches. Always create a new `V{N+1}__*.sql`.

## File Location

```
services/{name}/{short}-app-infrastructure/src/main/resources/db/migration/V{N}__{description}.sql
```

## MySQL Dialect Rules

Java services use MySQL only (no SQLite dual-version needed, unlike Rust's
`prue_backend`). Use standard MySQL DDL:

| Concept | MySQL syntax |
|---|---|
| Auto-increment PK | `id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY` |
| String | `VARCHAR(n)` or `TEXT` |
| Boolean | `TINYINT(1) NOT NULL DEFAULT 0` |
| Timestamp | `DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP` |
| Timestamp (auto-update) | `DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP` |
| Soft delete | `deleted_at DATETIME DEFAULT NULL` |
| JSON | `JSON` |
| Enum | `ENUM('val1','val2') NOT NULL DEFAULT 'val1'` |
| UUID | `VARCHAR(36) NOT NULL` |

## CREATE TABLE Template

```sql
-- V{N}__{description}.sql - {description} for {name}.
-- Table prefix `{prefix}` registered in scripts/check-db-naming.js.

SET NAMES utf8mb4;

CREATE TABLE {table} (
    id BIGINT NOT NULL AUTO_INCREMENT,
    -- {columns}
    name VARCHAR(128) NOT NULL,
    description TEXT,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_by VARCHAR(64) NOT NULL,
    deleted_at DATETIME DEFAULT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uk_{table}_name (name),
    KEY idx_{table}_status (status, deleted_at)
    -- {foreign_keys}
    -- CONSTRAINT fk_{table}_{ref_col} FOREIGN KEY ({ref_col}) REFERENCES {ref_table}(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='{description}';
```

## ALTER TABLE Templates

### Add column

```sql
-- V{N}__add_{column}_to_{table}.sql
ALTER TABLE {table}
    ADD COLUMN {column} {type} {nullable} {default} {after};
```

Example:
```sql
ALTER TABLE billing_invoices
    ADD COLUMN currency VARCHAR(3) NOT NULL DEFAULT 'CNY' AFTER amount;
```

### Add index

```sql
-- V{N}__add_index_on_{table}.sql
ALTER TABLE {table}
    ADD INDEX idx_{table}_{cols} ({col1}, {col2});
```

### Add unique key

```sql
ALTER TABLE {table}
    ADD UNIQUE KEY uk_{table}_{col} ({col});
```

### Add foreign key

```sql
ALTER TABLE {table}
    ADD CONSTRAINT fk_{table}_{ref_col} FOREIGN KEY ({ref_col}) REFERENCES {ref_table}(id);
```

## Standard Column Set

Every business table should include the audit + soft-delete columns:

| Column | Type | Purpose |
|---|---|---|
| `id` | `BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY` | Surrogate PK |
| `created_by` | `VARCHAR(64) NOT NULL` | Actor who created the row (from JWT `sub` / `Principal.userId` — **random string, not numeric**) |
| `deleted_at` | `DATETIME DEFAULT NULL` | Soft-delete marker (NULL = active) |
| `created_at` | `DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP` | Creation timestamp |
| `updated_at` | `DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP` | Last-modified timestamp |

> **User reference columns must be `VARCHAR(64)`** — the platform's user id is a
> random string (UUID / Nano ID), not an auto-increment integer. This applies to
> `user_id` / `created_by` / `updated_by` / `operator_id` / `actor_id` / `owner_id`.
> Never use `BIGINT` for these columns. Legacy tables may use `VARCHAR(32/36/128)`;
> do not migrate them, but all new tables/columns use 64.
> Authoritative spec: `contracts/db-conventions.md` §「用户引用字段类型约定」.

MyBatis-Plus `@TableLogic(value = "NULL", delval = "NOW()")` on `deleted_at`
makes `removeById()` / `delete()` perform a soft delete (sets `deleted_at` to
`NOW()`) instead of a hard `DELETE`. Queries automatically exclude soft-deleted
rows.

## Data Backfill Migration

For data migrations (not schema changes), use a transaction-safe approach:

```sql
-- V{N}__backfill_{description}.sql
-- Data backfill: {description}
-- Idempotent: safe to re-run (uses INSERT ... ON DUPLICATE KEY UPDATE or WHERE NOT EXISTS).

INSERT INTO {table} (name, description, status, created_by)
SELECT 'default', 'auto-seeded row', 'active', 'system'
WHERE NOT EXISTS (
    SELECT 1 FROM {table} WHERE name = 'default'
);
```

## Post-Migration Steps

1. **Update the entity** (if a new column was added): add the field to the
   corresponding entity class in `{short}-app-infrastructure/.../persistence/entity/`
   with matching `@TableField` annotation.

2. **Update the mapper** (if custom queries are needed): add methods to the
   `BaseMapper` extension in `.../persistence/mapper/`.

3. **Update the service** (if business logic changed): adjust the service class
   in `.../service/` to handle new fields.

4. **Verify locally**:
   ```bash
   cd services/{name}
   mvn -Pjava21 clean package -DskipTests
   java -jar {short}-app-bootstrap/target/{short}-app-bootstrap.jar --spring.profiles.active=dev
   # Flyway logs show: "Migrating schema `superpowers` to version {N} - {description}"
   ```

5. **CI gates** (from repo root):
   ```bash
   node scripts/check-db-naming.js    # table prefix validation
   node scripts/check-service-standard.js  # 8-item baseline
   ```

## No Down Migration

Flyway is **forward-only** by design. Unlike Rust's `coder-migrate-db` skill
(which provides `-- down` blocks), Java/Flyway does not support down migrations
in the community edition. To revert a schema change:

1. Create a new forward migration `V{N+1}__revert_{description}.sql` that
   reverses the operation (e.g. `DROP COLUMN`, `DROP TABLE`).
2. Never delete or edit an already-applied migration file.

> **Project policy** (`AGENTS.md` §数据库迁移): "默认 forward-only：不编辑已发布迁移，新增 V{N+1}__*.sql。项目处于开发阶段、无生产数据：可大胆重构。"

## Validation Checklist

- [ ] Filename is `V{N}__{description}.sql` with `{N}` being the next sequential number
- [ ] Table name starts with the service prefix (`{prefix}`)
- [ ] Index names follow `idx_<table>_<cols>` convention
- [ ] Foreign key names follow `fk_<table>_<col>` convention
- [ ] `id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY` is present (for new tables)
- [ ] `created_at` + `updated_at` + `deleted_at` audit columns are present (for new tables)
- [ ] `created_by VARCHAR(64) NOT NULL` is present (for new tables); all user reference columns are `VARCHAR(64)`, **not** `BIGINT`
- [ ] `SET NAMES utf8mb4;` at the top of the file
- [ ] `ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci` on CREATE TABLE
- [ ] `node scripts/check-db-naming.js` passes (from repo root)
- [ ] No existing migration file was edited

## Common Mistakes to Avoid

- ❌ Editing an existing migration file -> Flyway checksum mismatch, app refuses to start
- ❌ Forgetting the service prefix on table names -> `check-db-naming.js` CI gate fails
- ❌ Using `AUTO_INCREMENT` without `PRIMARY KEY` -> MySQL rejects the DDL
- ❌ Creating a foreign key to a table in a different service's prefix (e.g. `iam_users`) -> cross-service FKs break deployment ordering; store as a plain column without FK constraint (`VARCHAR(64)` if it references a user id, `BIGINT` otherwise)
- ❌ Using `TIMESTAMP` instead of `DATETIME` for audit columns -> `TIMESTAMP` has a 2038 limit and timezone quirks; use `DATETIME`
- ❌ Forgetting `ON UPDATE CURRENT_TIMESTAMP` on `updated_at` -> the column never auto-updates
- ❌ Using `JSON` type for indexed columns -> MySQL can't index JSON directly; extract to a generated column first
- ❌ Using `BIGINT` for `user_id` / `created_by` / `updated_by` / `operator_id` / `actor_id` / `owner_id` -> platform user id is a random string (UUID / Nano ID); use `VARCHAR(64)` instead (see `contracts/db-conventions.md` §「用户引用字段类型约定」)
