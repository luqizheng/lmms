# User Service 多数据库支持 — Design

日期: 2026-06-10
范围: `services/user-service/`（仅 Rust 版本，不动 `libs/common`、不动其他服务）

## 目的

为 `user-service` 增加 MySQL / MSSQL / SQLite / 达梦（DM）四种后端数据库的连接层支持。配置 `database.db_type` 决定启动时连哪种数据库，业务 SQL 保持现状（MySQL 方言），非 MySQL 路径在 service 层返回 `AppError::UnsupportedDb`，把"四库支持"做实在类型层。

达梦走 ODBC 通道（`odbc-api` crate + 系统 DM8 ODBC 驱动），与 sqlx 三库（MySQL/MSSQL/SQLite）共用 `DbPool` 枚举抽象。

## 范围

**在范围内**:
- `services/user-service/src/config.rs` 新增 `DbType` 枚举（含 `Dameng`）+ 字段
- `services/user-service/src/db.rs` 重写：`DbPool` 枚举（含 `Dameng` 变体）+ `init_pool` 按 db_type 分发
- `services/user-service/src/error.rs` 新增 `UnsupportedDb(DbType)` 变体 + 501 映射
- `services/user-service/Cargo.toml` sqlx features 加 `mssql` / `sqlite`；新增 `odbc-api` / `encoding_rs` 依赖
- service / routes 层所有 `&MySqlPool` 签名 → `&DbPool`，函数体加 `match`
- `services/user-service/config/config.yaml` 增 `db_type: mysql`，附 达梦 连接串示例
- 单测：db.rs / error.rs 各加 `#[cfg(test)] mod tests`
- 集成测试 `tests/integration_test.rs` 末尾增 `mod multi_db`
- 文档：`docs/superpowers/specs/...` 注明达梦需要系统装 DM8 ODBC 驱动

**不在范围内**:
- 业务 SQL 改写（`INSERT IGNORE` / `NOW()` 等保留 MySQL 方言）
- MSSQL / SQLite 的 DDL / 数据迁移脚本
- `libs/common` 改动（其他服务继续绑 MySQL）
- `services/auth-service` / `services/token-service` / `services/api-gateway` 改动
- `tests/` 现有用例改动

## 架构

### 组件图

```
config/config.yaml
   │  database.db_type
   ▼
Config { database: DatabaseCfg { db_type, url, max_connections } }
   │
   ▼
db::init_pool(&cfg.database)
   │  match cfg.db_type
   ├──► Mysql  → MySqlPoolOptions.connect(url)         → DbPool::Mysql(p)
   ├──► Mssql  → MssqlPoolOptions.connect(url)         → DbPool::Mssql(p)
   ├──► Sqlite → SqlitePoolOptions.connect(url)        → DbPool::Sqlite(p)
   └──► Dameng → odbc_api::Environment::new()
                  .connect_with_connection_string(url) → DbPool::Dameng(Arc<Mutex<Conn>>)
   │
   ▼
build_router(pool: DbPool)        // Axum State
   │
   ▼
routes/*.rs ──► service/*.rs       // 接收 &DbPool
                  match pool {
                      DbPool::Mysql(p) => 原 MySQL SQL,
                      _ => Err(AppError::UnsupportedDb(db_type))
                  }
```

### 改动文件

| 文件 | 改动 |
|---|---|
| `services/user-service/Cargo.toml` | sqlx features 加 `mssql` / `sqlite` |
| `services/user-service/src/config.rs` | 新增 `DbType` 枚举 + `database.db_type` 字段 |
| `services/user-service/src/db.rs` | 重写：暴露 `DbPool` 枚举 + `init_pool` |
| `services/user-service/src/error.rs` | 新增 `UnsupportedDb(DbType)` + 501 响应 |
| `services/user-service/src/main.rs` | 调用新 `init_pool`、日志输出 db_type |
| `services/user-service/src/service/user_svc.rs` | `&MySqlPool` → `&DbPool`，加 match |
| `services/user-service/src/service/role_svc.rs` | 同上 |
| `services/user-service/src/service/claim_svc.rs` | 同上 |
| `services/user-service/src/routes/health.rs` | match 三种池做 `SELECT 1` |
| `services/user-service/src/routes/users.rs` | `&MySqlPool` → `&DbPool` |
| `services/user-service/src/routes/internal.rs` | 同上 |
| `services/user-service/src/routes/permissions.rs` | 同上 |
| `services/user-service/src/routes/roles.rs` | 同上 |
| `services/user-service/src/routes/user_roles.rs` | 同上 |
| `services/user-service/config/config.yaml` | 增 `db_type: mysql` |
| `services/user-service/tests/integration_test.rs` | 末尾增 `mod multi_db` |

## 数据流

### 启动

```rust
let cfg = load_config("config/config.yaml")?;
let pool = init_pool(&cfg.database).await?;
tracing::info!(db_type = ?cfg.database.db_type, "user-service started");
let app = build_router(pool, ...);
axum::Server::bind(&addr).serve(app.into_make_service()).await?;
```

### 请求

```rust
// routes/users.rs
pub async fn get_user(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<Json<UserDTO>, AppError> {
    user_svc::fetch_user_with_roles(&pool, id).await.map(Json)
}
```

```rust
// service/user_svc.rs
pub async fn fetch_user_with_roles(
    pool: &DbPool,
    user_id: u64,
) -> Result<UserDTO, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let row = sqlx::query_as::<_, UserRow>(
                "SELECT id, username, email, phone, nickname, status, user_code, \
                        profile_json, created_at, updated_at \
                 FROM users WHERE id = ? AND status = 1",
            )
            .bind(user_id)
            .fetch_optional(p)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("user id={user_id}")))?;
            let roles = fetch_user_role_codes(pool, user_id).await?;
            Ok(build_user_dto(row, roles))
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}
```

### 健康检查

```rust
match &pool {
    DbPool::Mysql(p) => sqlx::query("SELECT 1").execute(p).await.is_ok(),
    DbPool::Mssql(p) => sqlx::query("SELECT 1").execute(p).await.is_ok(),
    DbPool::Sqlite(p) => sqlx::query("SELECT 1").execute(p).await.is_ok(),
}
```

## 错误处理

### 新增变体

```rust
// src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // ... 现有变体 ...

    #[error("db type {0:?} not supported for this operation")]
    UnsupportedDb(DbType),
}
```

### HTTP 映射

```rust
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::UnsupportedDb(t) => (
                StatusCode::NOT_IMPLEMENTED,
                Json(json!({"error": "unsupported_db", "db_type": format!("{:?}", t)})),
            ).into_response(),
            // ... 现有分支 ...
        }
    }
}
```

### 启动期校验

- `cfg.database.db_type == Mysql` 且 `url` 不是 `mysql://` → sqlx `connect` 失败，anyhow 冒泡
- `cfg.database.db_type == Sqlite` 且 url 含目录路径且目录不存在 → sqlx `connect` 失败
- `cfg.database.db_type == Dameng` 且系统未装 DM8 ODBC 驱动 → `Environment::new()` 或 `connect_with_connection_string` 失败，anyhow 冒泡
- 启动不重复校验 db_type 与 url scheme 一致（依赖 sqlx / odbc-api 自带）

## 配置

```yaml
# config/config.yaml
server:
  host: "0.0.0.0"
  port: 8082
database:
  db_type: mysql              # 新增字段，可选值: mysql | mssql | sqlite | dameng
  url: "mysql://user:pass@localhost:3306/kudi"
  max_connections: 16
redis:
  url: "redis://localhost:6379"
auth:
  service_token_issuer: "token-service"
  service_token_audience: "user-service"
```
DbType 枚举:
```rust
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DbType { Mysql, Mssql, Sqlite, Dameng }
```

DbPool 枚举:
```rust
#[derive(Clone)]
pub enum DbPool {
    Mysql(MySqlPool),
    Mssql(MssqlPool),
    Sqlite(SqlitePool),
    Dameng(Arc<Mutex<odbc_api::Connection<'static>>>),
}

impl DbPool {
    pub fn db_type(&self) -> DbType {
        match self {
            Self::Mysql(_)  => DbType::Mysql,
            Self::Mssql(_)  => DbType::Mssql,
            Self::Sqlite(_) => DbType::Sqlite,
            Self::Dameng(_) => DbType::Dameng,
        }
    }
}
```

注：达梦走 `Arc<Mutex<odbc_api::Connection>>` 单连接模型（odbc-api 无连接池）。并发请求在同一连接上串行化，吞吐量受限，但贴原参考文章实现、最小化抽象。

## 依赖

`Cargo.toml` 改动:
```toml
sqlx = { version = "0.8", default-features = false,
         features = ["runtime-tokio-rustls", "mysql", "mssql", "sqlite"] }
odbc-api = { version = "8.1", features = ["derive"] }
encoding_rs = "0.8"
```

## 系统要求（达梦）

达梦分支要求运行机器已装 DM8 ODBC 驱动并注册到系统 ODBC 管理器：
- Windows：安装 `DM8 ODBC DRIVER`（达梦安装包勾选 ODBC 组件）
- Linux：`libodbc` + 达梦 ODBC `.so` + `odbcinst.ini` 注册
- 启动时若未装驱动，`odbc_api::Environment::new()` 或 `connect_with_connection_string` 失败，anyhow 冒泡

`config.yaml` 的 `url` 字段在 `db_type: dameng` 时填 ODBC 连接串，例如:
```yaml
database:
  db_type: dameng
  url: "Driver={DM8 ODBC DRIVER};Server=127.0.0.1;Port=5236;UID=SYSDBA;PWD=xxx;Schema=DMHR"
  max_connections: 1
```
注：达梦 `max_connections` 实际无意义（单连接），保留字段仅为配置统一。

## 测试

### 单元测试（`#[cfg(test)]` 旁路）

`src/db.rs`:
- `parse_db_type_mysql` / `parse_db_type_mssql` / `parse_db_type_sqlite` / `parse_db_type_dameng` —— yaml 解析正确
- `parse_db_type_missing` —— 缺字段 → serde error
- `parse_db_type_invalid` —— 未知值 → serde error

`src/error.rs`:
- `unsupported_db_status_is_501` —— IntoResponse 输出 501
- 响应体 JSON 含 `"error":"unsupported_db"` 和 `"db_type":"dameng"`

### 集成测试（`tests/integration_test.rs` 末尾 `mod multi_db`）

- 复用 testcontainers 现有 MySQL 容器，验证 `init_pool` 拿到 `DbPool::Mysql(_)`，且 `pool.db_type() == Mysql`
- 不为 MSSQL / SQLite / Dameng 起容器（避免 CI 慢/无网络）
- `AppError::UnsupportedDb(Dameng).into_response().status() == 501`
- 达梦配置解析：`db_type: dameng` + ODBC 连接串 → 解析成功（不实际连，仅 yaml 解析）

### 不动的事

- `tests/` 下现有用例零改动
- `libs/common` 零改动
- 其他服务零改动

### 验证命令

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
# 真实 PowerShell（vcvarsall x64）下：
cargo test -p user-service
```

## 不做的事

- ❌ 不改业务 SQL（`INSERT IGNORE` / `NOW()` / `?` 占位符全部保留）
- ❌ 不写 MSSQL / SQLite 的 DDL 迁移脚本
- ❌ 不动 `libs/common` 的 `db::connect`
- ❌ 不为 MSSQL / SQLite 起 testcontainer
- ❌ 不主动 commit（用户明说才 commit）
- ❌ 不发 SDK / 不打 tag
