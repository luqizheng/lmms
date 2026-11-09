# Service-to-Service Authentication (Phase 5-B)

> **Status**: Active since 2026-07-07 (phase5b). Supersedes Consul Connect mTLS (phase5a, deprecated).

## Overview

After phase5a-2 (dephase Consul Connect), service-to-service authn moved from mTLS sidecar to **JWT-based**:

- **User JWT**: gateway issues after login; `aud: "user"`; verified by all services via JWKS
- **Service JWT**: token-service issues via `POST /tokens/service-jwt`; `aud: "<target>"`; used for internal calls (notification→iam, token→iam)

## Architecture

```
[Startup]
  service → GET JWKS from token-service:8082/.well-known/jwks.json
  service → POST /tokens/service-jwt (with bootstrap token) → cached service-jwt

[Inbound request]
  Authorization: Bearer <user-jwt or service-jwt>
  → Principal extractor (jwt-ctx) verifies signature via JWKSCache
  → /iam/internal/* also requires aud="iam-service" (verify_aud middleware)

[Outbound call]
  ResilientClient or manual header injection
  → Authorization: Bearer <service-jwt for target>
```

## Bootstrap Tokens

Each business service has a pre-shared `SERVICE_BOOTSTRAP_TOKEN` (32-byte random). At startup, the service exchanges it for a short-lived service-jwt (1h TTL) via `POST /tokens/service-jwt`.

### 存储位置（2026-11-09 起）

token-service 把 bootstrap token 持久化到 **`token_bootstrap_tokens`** 表（V003 迁移）：

| 列 | 说明 |
|---|---|
| `owner` | 主键。服务名，签出的 JWT 的 `sub` 即此值 |
| `token_hash` | 当前存明文；列名预留以便后续切 SHA-256 |
| `status` | `active` / `disabled`；非 active 一律拒绝 |
| `expires_at` | 可空。NULL = 永久 |
| `last_used_at` | 最近命中时间（预留运维审计） |

启动时自动 seed（`models/bootstrap_token.rs::seed_defaults`）：

- **来源优先级**：env `SERVICE_BOOTSTRAP_TOKENS` + `SERVICE_BOOTSTRAP_OWNERS` → 内置 dev 默认值 `DEFAULT_DEV_BOOTSTRAP`（两个 env 都为空时才用）。
- **fail-closed**：env 提供了 tokens 但 owners 缺失 / 长度不匹配 → **token-service 拒绝启动**，不会静默回退到内置默认值（那会让生产环境用上硬编码 dev token）。
- **幂等 + 不覆盖**：按 `owner` 补插——表里已有该 owner 则不动。因此运维 rotate 过的 token 不会被重启冲掉。

### 新增一个服务时怎么加 token

两种方式，**推荐 B**（更省事）：

**A. 直接写 DB**（临时/一次性，立即生效，不用重启 token-service）
```sql
INSERT INTO token_bootstrap_tokens (owner, token_hash, status)
VALUES ('my-new-service', 'dev-bt-my-new-service', 'active');
```

**B. 改配置 + 重启 token-service**（正式做法）
1. `services/token-service/src/models/bootstrap_token.rs` 的 `DEFAULT_DEV_BOOTSTRAP` 加一行 `("my-new-service", "dev-bt-my-new-service")`
2. `infrastructure/lmms-dev/.env`（及 `.env.example`、`.env.postgres.example`）加两项，**两个 CSV 必须同时改且等长、顺序一一对应**：
   ```bash
   MY_NEW_DEV_BT=dev-bt-my-new-service
   SERVICE_BOOTSTRAP_TOKENS=...,dev-bt-my-new-service
   SERVICE_BOOTSTRAP_OWNERS=...,my-new-service
   ```
3. 业务服务自己的 env 加 `SERVICE_BOOTSTRAP_TOKEN: "${MY_NEW_DEV_BT}"`（docker-compose + `.env`）
4. 重启 token-service（seed 生效）+ 重启新服务

> ⚠️ 只改 `SERVICE_BOOTSTRAP_TOKENS` 而漏改 `SERVICE_BOOTSTRAP_OWNERS`（或长度不等）→ token-service 拒绝启动。这是刻意的 fail-closed。

### 管理端点（Phase 3）

两套路由共用同一份业务逻辑（`routes/bootstrap_token_admin.rs` 的 `ops` 模块）：

| 路由前缀 | 鉴权 | 用途 |
|---|---|---|
| `/admin/bootstrap-tokens*` | service-jwt（`aud=token-service`） | CI / 运维脚本 |
| `/v1/admin/bootstrap-tokens*` | user-jwt + 权限码 | **Web UI（浏览器）** |

Web UI 走 api-gateway（`token.router.toml` 已配 `/v1/admin/bootstrap-tokens*` → token-service 同名路径，protected = 需登录）。

**权限码**（`contracts/permissions.yaml` → `libs/authz/src/perms.rs`）：

| 权限码 | 覆盖操作 |
|---|---|
| `TOKEN_BOOTSTRAP_READ` | 列表 / 查单条 |
| `TOKEN_BOOTSTRAP_CREATE` | 新建 |
| `TOKEN_BOOTSTRAP_UPDATE` | rotate / 启停 |
| `TOKEN_BOOTSTRAP_DELETE` | 删除 |

| 端点 | 方法 | 所需权限码 | 说明 |
|---|---|---|---|
| `/v1/admin/bootstrap-tokens` | GET | READ | 列表（**不含明文 token**）；支持 `?keyword=` `?status=` 过滤 |
| `/v1/admin/bootstrap-tokens` | POST | CREATE | 新建，返回明文一次（owner 已存在 → 409） |
| `/v1/admin/bootstrap-tokens/{owner}` | GET | READ | 查单条元数据（不含明文） |
| `/v1/admin/bootstrap-tokens/{owner}` | DELETE | DELETE | 删除（含宽限期遗留行） |
| `/v1/admin/bootstrap-tokens/{owner}/rotate` | POST | UPDATE | 换新 token，返回明文一次；body `{"grace_period_sec": N}` |
| `/v1/admin/bootstrap-tokens/{owner}/status` | PUT | UPDATE | 启用 / 禁用；body `{"status": "active\|disabled"}` |

权限码不足 → **403**；缺 Bearer → 401。

**rotate 的宽限期**（body 里的 `grace_period_sec`）：

| N | 行为 |
|---|---|
| `0`（默认） | 旧 token **立即失效** |
| `> 0` | 旧 token 保留到 `now + N` 秒——**新旧 token 并存**，服务可逐个重启平滑切换 |

宽限期实现：旧 token 另起一行（owner 加 `#old` 后缀）存放，带 `expires_at`；到期后校验 SQL 自动拒绝，`purge_expired_grace_rows` 可清理残留。

**Web UI 调用示例**（经网关，`Authorization` 传登录后的 user-jwt）：
```bash
# 列表
curl http://api-gateway:3000/v1/admin/bootstrap-tokens \
  -H "Authorization: Bearer <user-jwt>"

# 轮换（给旧 token 5 分钟宽限期）
curl -X POST http://api-gateway:3000/v1/admin/bootstrap-tokens/iam-service/rotate \
  -H "Authorization: Bearer <user-jwt>" \
  -H "Content-Type: application/json" \
  -d '{"grace_period_sec": 300}'
```

> ⚠️ 列表 / 查询**不返回明文 token**（当前 `token_hash` 列存的就是明文，DTO 刻意不带该字段）。create / rotate 的响应里明文**只出现那一次**，DB 里没有第二份——调用方需立刻保存。

**生产**：用 secret manager（Vault、K8s Secret）注入真实随机值，**不要提交真实 token**。

## Key Components

| Component | Location | Role |
|---|---|---|
| `libs/digua_common-jwt` | JWKSCache, verify_token, sign_token, ServiceJwtCache | Crypto primitives |
| `libs/jwt-ctx` | Principal::from_token (async, verify) | Extractor for Axum |
| `libs/digua_authz` | verify_aud_principal_middleware | Route-level aud gate |
| `token-service` | POST /tokens/service-jwt | Minting endpoint |
| Each service main.rs | JWKS + ServiceJwtCache at startup | Cold-start init |

## Env Vars (per service)

| Var | Required | Default | Description |
|---|---|---|---|
| `SERVICE_BOOTSTRAP_TOKEN` | Yes (except token-service) | — | Pre-shared secret for /tokens/service-jwt |
| `JWKS_URL` | No | `http://token-service:8082/.well-known/jwks.json` | JWKS endpoint |
| `TOKEN_SERVICE_URL` | No | `http://token-service:8082` | token-service base URL |
| `TENANT_ID` | No | `1` | Default tenant for service-jwt claims |
| `SERVICE_BOOTSTRAP_TOKENS` | token-service only | 内置 dev 默认值 | Comma-separated accepted tokens（seed 用） |
| `SERVICE_BOOTSTRAP_OWNERS` | token-service only | 内置 dev 默认值 | 与上面等长、顺序一一对应的 owner 列表 |

## Error Handling

| Scenario | HTTP Status | Behavior |
|---|---|---|
| Missing Authorization header | 401 | Principal extractor rejects |
| Invalid signature | 401 | verify_token returns SignatureInvalid |
| Expired JWT | 401 | verify_token returns Expired |
| Wrong audience | 401 | verify_aud middleware rejects |
| JWKS unreachable at startup | panic | Service cannot start without verification keys |
| Service-jwt refresh failure | retry w/ backoff | 1s/2s/4s/.../60s cap |

## Adding a New Internal Endpoint

1. Add route to service's router
2. Wrap with `verify_aud_principal_middleware::<AppState>("<service-name>")`
3. Handler uses `Principal` extractor for identity
4. Outbound calls inject `Authorization: Bearer <service-jwt>` via `ServiceJwtCache::get("<target>")`

```rust
let internal = internal_routes::routes(state.clone())
    .layer(axum::middleware::from_fn_with_state(
        state.clone(),
        verify_aud_principal_middleware::<AppState>("my-service"),
    ));
```
