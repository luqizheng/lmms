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

## 用户引用字段类型约定

**所有引用用户身份的列统一使用 `VARCHAR(64)`**，因为本平台 user id 是随机字符串（UUID / Nano ID 等），不是自增整数。适用字段：

| 字段 | 含义 |
| --- | --- |
| `user_id` | 用户主键引用 / 外键 |
| `created_by` / `updated_by` | 审计字段（操作人） |
| `operator_id` / `actor_id` / `owner_id` | 操作者 / 行为者 / 所有者 |

规则：

- MySQL：`VARCHAR(64)`（`NOT NULL` 或 `NULL` 视语义，索引照常建）。
- SQLite：`TEXT`（SQLite 无 VARCHAR，与 `coder-migrate-db` 类型映射一致）。
- **禁止**对这些列使用 `BIGINT` / `INT` —— 会跟 JWT `sub`（随机字符串）对不上，写入即失败或静默截断。
- 现存历史表长度不一（`VARCHAR(32/36/64/128)`）**不在本次强制范围内**；本约定只约束**新增表 / 新增列**。
- 关联：`iam_users.id` 当前为 `VARCHAR(36)`（UUID），新表引用它时仍写 `VARCHAR(64)`（64 是上限，向后兼容 36 长度的值）。

## 不在规范内

- 除「用户引用字段」之外的列名、列类型、索引语义。
- 跨服务字段命名（每个服务自行约定）。
- 第三方组件自带表（sqlx migration metadata、`_sqlx_migrations` 等）。

## CI 检查

`node scripts/check-db-naming.js` 会扫：
- `services/*/migrations/**/*.sql`：所有 `CREATE TABLE` / `ALTER TABLE` 的表名必须以服务前缀起头。
- `services/*/src/**/*.rs`：所有 sqlx 字符串里的 `FROM` / `JOIN` / `INTO` / `UPDATE` / `DELETE FROM` 后跟随的表名必须以服务前缀起头。
- 例外白名单维护在脚本顶部（`_sqlx_migrations`、`information_schema`、`sqlite_master` 等）。

## 历史背景

此规范在 Phase 4.0 落地（`docs/superpowers/specs/2026-06-26-phase4.0-table-naming-design.md`）。落地前所有业务表均无前缀，引发 iam/audit 之间的 `login_logs` 撞名 bug。
