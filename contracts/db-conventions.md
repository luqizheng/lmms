# Database Naming Conventions

## 背景

所有 Rust 服务连接同一个 MySQL 实例的同一个 database `superpowers`。为防止跨服务表名冲突，统一约定如下规范。SQLite（prue_backend 单机 dev 模式）虽无共库问题，但同样遵循此规范以保持代码一致。

历史上 iam-service 与 audit-service 均声明了 `login_logs`，且 schema 完全不同（PK 类型 / 字段集 / 索引都不一样），同库内只能有一张物理表存在，另一边的写入一直在静默失败。本规范的强制前缀消除了此类隐患。

## 表名前缀映射

| 服务（crate）         | 前缀     | 示例                              |
| --------------------- | -------- | --------------------------------- |
| iam-service           | `iam_`   | `iam_users`、`iam_roles`          |
| audit-service         | `audit_` | `audit_login_logs`                |
| token-service         | `token_` | `token_refresh_markers`           |
| notification-service  | `ntf_`   | `ntf_announcements`               |
| config-service        | `cfg_`   | `cfg_namespaces`                  |
| file-service          | `file_`  | `file_files`                      |
| prue_backend          | `prue_`  | `prue_menu_configs`               |

> 与 `contracts/error-codes.yaml` 中的服务缩写（IAM/CFG/AUD/NTF/FILE/SYS）**故意不完全对齐**：错误码追求 3 字母短，表名追求可读，各自规则独立校验。

## 命名规则

- 表名：`<prefix>_<resource_plural_snake_case>`。例：`iam_users`、`audit_login_logs`、`ntf_announcement_recipients`。
- 索引名：`idx_<table>_<cols>`。例：`idx_iam_users_username`。
- 唯一键：`uk_<table>_<cols>`。例：`uk_iam_users_email`。
- 外键名：`fk_<table>_<col>`。例：`fk_iam_user_roles_user_id`。

## 不在规范内

- 列名、列类型、索引语义。
- 跨服务字段命名（每个服务自行约定）。
- 第三方组件自带表（sqlx migration metadata、`_sqlx_migrations` 等）。

## CI 检查

`node scripts/check-db-naming.js` 会扫：
- `services/*/migrations/**/*.sql`：所有 `CREATE TABLE` / `ALTER TABLE` 的表名必须以服务前缀起头。
- `services/*/src/**/*.rs`：所有 sqlx 字符串里的 `FROM` / `JOIN` / `INTO` / `UPDATE` / `DELETE FROM` 后跟随的表名必须以服务前缀起头。
- 例外白名单维护在脚本顶部（`_sqlx_migrations`、`information_schema`、`sqlite_master` 等）。

## 历史背景

此规范在 Phase 4.0 落地（`docs/superpowers/specs/2026-06-26-phase4.0-table-naming-design.md`）。落地前所有业务表均无前缀，引发 iam/audit 之间的 `login_logs` 撞名 bug。
