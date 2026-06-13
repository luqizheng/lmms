# libs/ 共享内核

| crate | 职责 | 关键依赖 |
|---|---|---|
| common-error | AppError + axum IntoResponse | thiserror / axum |
| common-log | tracing JSON + request_id 中间件 | tracing-subscriber |
| common-config | 配置加载（env + file） | config |
| common-db | sqlx 多方言（mysql/dameng/mssql/sqlite）+ SqliteFixture | sqlx + tempfile |
| common-cache | Cache trait + MemoryCache(Moka) / Redis(可选) | moka / redis |
| common-http | reqwest 封装 + 请求 ID 透传 | reqwest |
| common-crypto | Argon2 + AES-256-GCM | argon2 / aes-gcm |
| common-validator | 中国手机号 / 强密码等 | validator |
| common（外观） | 兼容旧路径，转发到新 crate | — |
| jwt-ctx / authz | JWT 上下文 + RBAC | (已存在) |

## feature 矩阵

- `common-db`：`mysql` / `dameng` / `mssql` / `sqlite` / `test-kit`，互不强制。
- `common-cache`：`memory`（默认）/ `redis`。

## 单测约定

- 用 `common-db` 的 `SqliteFixture` 做 DB 单测；每用例独立 tempfile，并行安全。
- 全部 cargo 命令在真实 PowerShell（vcvarsall x64）跑。