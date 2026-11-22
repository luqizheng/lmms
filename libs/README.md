# libs/ 共享内核

| crate | 职责 | 关键依赖 |
|---|---|---|
| digua_common-error | AppError + axum IntoResponse | thiserror / axum |
| digua_common-log | tracing JSON + request_id 中间件 | tracing-subscriber |
| digua_common-config | 配置加载（env + file） | config |
| digua_common-db | sqlx 多方言（mysql/postgres/sqlite）+ SqliteFixture | sqlx + tempfile |
| digua_common-cache | Cache trait + MemoryCache(Moka) / Redis(可选) | moka / redis |
| digua_common-http | reqwest 封装 + 请求 ID 透传 | reqwest |
| digua_common-crypto | Argon2 + AES-256-GCM | argon2 / aes-gcm |
| digua_common-validator | 中国手机号 / 强密码等 | validator |
| digua_common_consul | Consul 自注册（注册/对账/SIGTERM 注销；替代 registrator） | reqwest / prometheus |
| digua_common（外观） | 兼容旧路径，转发到新 crate | — |
| jwt-ctx / digua_authz | JWT 上下文 + RBAC | (已存在) |

## feature 矩阵

- `digua_common-db`：`mysql` / `postgres` / `sqlite` / `test-kit`，互不强制。
- `digua_common-cache`：`memory`（默认）/ `redis`。

## 单测约定

- 用 `digua_common-db` 的 `SqliteFixture` 做 DB 单测；每用例独立 tempfile，并行安全。
- 全部 cargo 命令在真实 PowerShell（vcvarsall x64）跑。