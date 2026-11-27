# token-service

OAuth 2.0 Token 服务，负责 Access Token、Refresh Token、API Key 的颁发与管理。

## 功能特性

### OAuth 2.0 授权码模式
- **Access Token** - 基于授权码交换获取访问令牌
- **Refresh Token** - 刷新访问令牌，支持防重放攻击
- **Revoke** - 主动撤销令牌，加入黑名单

### 资源令牌
- **Resource Token** - 为特定资源生成受限访问令牌
- 支持自定义 TTL 和权限范围

### 用户 JWT
- **User JWT** - 为用户生成身份令牌
- 支持自定义声明和过期时间

### API Key 管理
- **API Key 创建** - 创建 OAuth 客户端凭证
- 支持配置权限列表和令牌有效期

### 安全特性
- 使用 Argon2 进行客户端密钥哈希存储
- JWT 使用 RS256 非对称加密签名
- Refresh Token 单次使用机制（防重放）
- 客户端状态管理（启用/禁用）

### 服务能力
- **服务发现** - 集成 Consul 自动注册
- **健康检查** - `/health` 端点
- **指标监控** - Prometheus `/metrics`
- **API 文档** - Swagger UI

## 技术栈

- Rust 1.78+
- Axum 0.8（Web 框架）
- SQLx（数据库 ORM）
- JWT-Simple（JWT 处理）
- Consul（服务发现）
- Prometheus（指标监控）
- Utoipa（OpenAPI 文档）

## 配置

配置文件：`config/config.yaml`

```yaml
server:
  host: 0.0.0.0
  port: 4000

issuer: superpowers-token-service

database:
  db_type: mysql
  url: mysql://token_service:token_pwd@localhost:3306/token_service
  max_connections: 16

jwt_private_key_path: keys/private.pem
jwt_public_key_path: keys/public.pem

default_access_ttl_sec: 3600
default_refresh_ttl_sec: 2592000
default_resource_ttl_sec: 300

user_service_url: http://user-service:8080

# service-jwt grant 管理主体清单（逗号分隔 canonical owner）。
# 空 = fail-closed：任何 service-jwt 调 /admin/service-jwt-grants* 都 403。
service_jwt_grant_admins: ["iam-service"]

consul:
  addr: http://localhost:8500
  service_name: token-service
  service_id_file: /tmp/token-service-id
```

## 配置说明

| 配置项 | 说明 | 默认值 |
|--------|------|--------|
| server.host | 服务绑定地址 | 0.0.0.0 |
| server.port | 服务端口 | 4000 |
| issuer | JWT issuer 标识 | superpowers-token-service |
| database.db_type | 数据库类型 | mysql |
| database.url | 数据库连接 URL | mysql://... |
| database.max_connections | 最大连接数 | 16 |
| jwt_private_key_path | 私钥路径 | keys/private.pem |
| jwt_public_key_path | 公钥路径 | keys/public.pem |
| default_access_ttl_sec | Access Token 默认有效期(秒) | 3600 |
| default_refresh_ttl_sec | Refresh Token 默认有效期(秒) | 2592000 |
| default_resource_ttl_sec | Resource Token 默认有效期(秒) | 300 |
| user_service_url | 用户服务地址 | http://user-service:8080 |
| service_jwt_grant_admins | 可调 `/admin/service-jwt-grants*` 的 service-jwt 主体清单（env `TOKEN_SERVICE_JWT_GRANT_ADMINS`）；空 = fail-closed 全拒 | 空 |
| consul.addr | Consul 地址 | http://localhost:8500 |
| consul.service_name | 服务名称 | token-service |

## 数据库迁移

迁移文件：`migrations/20260611000001_init.sql`

## 密钥生成

```bash
# 生成 RSA 密钥对
openssl genrsa -out keys/private.pem 2048
openssl rsa -in keys/private.pem -pubout -out keys/public.pem
```

## 构建

```bash
cargo build --manifest-path services/token-service/Cargo.toml
```

## 运行

```bash
cargo run --manifest-path services/token-service/Cargo.toml
```

## Docker

```bash
docker build -t token-service:latest services/token-service/
docker run -p 4000:4000 token-service:latest
```

## API 端点

| 方法 | 路径 | 描述 |
|------|------|------|
| GET | /health | 健康检查 |
| GET | /metrics | Prometheus 指标 |
| GET | /swagger-ui | Swagger UI 文档 |
| GET | /api-docs/openapi.json | OpenAPI 规范 |
| POST | /api/tokens/access | 获取 Access Token |
| POST | /api/tokens/refresh | 刷新 Token |
| POST | /api/tokens/revoke | 撤销 Token |
| POST | /api/tokens/resource | 获取 Resource Token（service-jwt 鉴权） |
| GET | /api/tokens/.well-known/jwks.json | JWKS |
| GET | /.well-known/jwks.json | JWKS（别名） |
| POST | /api/api-keys | 创建 API Key |
| POST | /api/v1/tokens/user-jwt | 获取用户 JWT |

## API 使用示例

### 获取 Access Token

```bash
curl -X POST http://localhost:4000/api/tokens/access \
  -H "Content-Type: application/json" \
  -d '{
    "grant_type": "authorization_code",
    "code": "auth_code_here",
    "client_id": "client_id",
    "client_secret": "client_secret"
  }'
```

### 刷新 Token

```bash
curl -X POST http://localhost:4000/api/tokens/refresh \
  -H "Content-Type: application/json" \
  -d '{
    "refresh_token": "refresh_token_here",
    "client_id": "client_id",
    "client_secret": "client_secret"
  }'
```

### 创建 API Key

```bash
curl -X POST http://localhost:4000/api/api-keys \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-api-key",
    "permissions": ["read", "write"],
    "grant_types": ["client_credentials"],
    "access_token_ttl_sec": 3600
  }'
```

## 测试

```bash
cargo test --manifest-path services/token-service/Cargo.toml
```
## JWT 签名密钥：持久化、轮换、备份与恢复（2026-11-23）

Spec：`docs/superpowers/specs/2026-11-23-token-jwt-key-persistence-design.md`（PR #7）
运维加固 spec：`docs/superpowers/specs/2026-11-23-token-jwt-key-operations-hardening-design.md`（PR #9）
运维手册（含泄露应急/故障处理）：`docs-site/runbooks/jwt-key-operations.md`

### 存储模型

单一真相源是 **`keystore.json`**（默认与 `TOKEN_JWT_PRIVATE_KEY_PATH` 同目录，
容器内即 `/app/keys/keystore.json`，可用 `TOKEN_JWT_KEYSTORE_PATH` 覆盖）。
文件包含全部密钥条目（active / retiring / retired）、各自的 PEM 与时间戳。

- **kid**：新钥 = 公钥 DER 的 SHA-256 前 16 字节 hex（内容寻址，跨重启稳定）；
  从旧 `private.pem`/`public.pem` 导入的密钥保留 `kid="default"`，
  保证升级前签发的 token 头部 kid 继续命中。
- **原子写**：`.tmp` + fsync + rename，中断不会产生半写文件。
- **权限**：文件 0600、目录 0700（unix）。加载时 group/other 可读 → **拒绝启动**。
- **fail-closed**：keystore.json 存在但损坏（JSON/PEM/公私钥不匹配）→ 启动失败并退出，
  **绝不静默重新生成**（旧行为是容器重建后重生成密钥导致全员掉线的根因）。

### 启动解析顺序（`KeyStore::init`）

1. `keystore.json` 存在 → 加载（任何损坏都硬失败）；
2. 不存在、旧 PEM 对存在 → 一次性导入（kid 保持 `default`）并原子落盘；
3. 两者都无 → 首次部署，生成新密钥并落盘；
4. 旧 PEM 只剩一半 → 拒绝启动（防止覆盖残缺密钥）。

### 已有部署升级（迁移）

- compose 已挂 `token-keys` named volume（`/app/keys`）。升级镜像后首次启动：
  volume 为空 + 容器内无旧 PEM → 自动生成新钥（旧 token 本就随旧容器丢了，无影响）。
- 若旧容器 volume 里已有 `private.pem`/`public.pem` → 自动导入，`kid="default"` 保留，
  **旧 JWT 全部继续可验证，无需重新登录**。
- 之前泄露在 git 历史中的 `services/token-service/keys/private.pem` 是开发密钥；
  生产部署务必使用首次启动生成（或备份恢复）的密钥，不要拷入该文件。

### 轮换（rotation）

```
active ──rotate──▶ retiring ──retire──▶ retired ──delete(≥保留期)──▶ 删除
 (签发)            (仅验证,在JWKS)     (不在JWKS)
```

- 在线：`POST /admin/keys/rotate`（service-jwt `aud=token-service`）。
  旧钥转 retiring 仍在 JWKS → 未过期旧 token 继续可验证，**无人被登出**。
  新签发 token 的 header kid 立即切到新钥（issuer 原子换钥）。
- 离线：`keytool rotate --keystore /app/keys/keystore.json`（停服窗口内执行，之后重启）。
- 退役：确认该 kid 签发的 token 已全部过期后 `POST /admin/keys/{kid}/retire`
  （或 `keytool retire`）——密钥移出 JWKS，验证方随后拒绝其 token。
- 删除：仅 `retired` 且超过 `TOKEN_JWT_MIN_RETENTION_SEC`（默认 30 天 = refresh TTL）
  才允许 `DELETE /admin/keys/{kid}`；未到保留期返回 409。
- 轮换节奏建议：每 90 天例行一次；密钥疑似泄露立即轮换 + 提前 retire。

### 备份与恢复

```bash
# 备份（容器内执行；输出文件含全部私钥，0600）
docker exec <token-container> /app/keytool backup \
  --keystore /app/keys/keystore.json --out /app/keys/keystore.backup.json
docker cp <token-container>:/app/keys/keystore.backup.json /secure/vault/   # 移出容器保存

# 恢复（先停服；校验通过才原子覆盖）
docker cp /secure/vault/keystore.backup.json <token-container>:/tmp/
docker exec <token-container> /app/keytool restore \
  --backup /tmp/keystore.backup.json --keystore /app/keys/keystore.json
docker restart <token-container>
```

- 恢复后重启，恢复前签发的 JWT 全部恢复可验证（单测 `backup_restore_preserves_old_tokens`）。
- 备份文件权限 0600，`*.backup.json` 已在 .gitignore；**严禁提交真实备份到 Git**。
- 备份节奏：每次 rotate 后、以及定期（如每周）备份一次到密管存储。

PR #9 加固后的备份/恢复语义：

- 备份是**信封格式**：`backup_format_version` / `created_at` / `producer` /
  `keystore_sha256` / `keystore`。恢复先校验信封版本，再校验 SHA-256 完整性，
  最后才做 PEM/公私钥全量校验；旧版裸 keystore 备份会被明确拒绝。
- 恢复顺序：校验 → 跨进程锁 → 写恢复前快照 `keystore.pre-restore.<epoch>.json`（0600）→
  指纹校验下原子覆盖 → 从磁盘读回校验；读回失败自动用快照回滚并报 `RestoreFailed`。
- 恢复期间若有并发写入（另一个 keytool / 副本），后写者收到 `Stale` 拒绝，现场不被覆盖。

### 密钥生命周期审计（PR #9）

所有 create/import/rotate/retire/delete/backup/restore（含失败）都追加写入
`keystore-audit.jsonl`（与 keystore 同目录，0600，逐条 fsync）：

```bash
docker exec <token-container> /app/keytool audit --keystore /app/keys/keystore.json --limit 50
```

- 记录与 `audit_events` 行同构：`event_id / event_type(token.jwt_key.*) / occurred_at /
  trace_id / producer / payload{actor, action, result, kid, related_kid, detail}`；
- 在线操作 `actor` = service-jwt `sub`，离线 `actor` = `keytool`，首启生成 = `system`；
- **绝不写私钥/PEM/凭证**（单测断言账本不含 `BEGIN`/PEM）；
- 写入失败打 `ERROR` 并累加 `token_keystore_audit_write_failures_total`（非 0 需处理）；
- 该账本暂未投递 audit-service（离线 keytool 无 DB/MQ、文件状态无法与 DB 事务原子），
  后续接入事件管道见 spec §2.1 follow-up。

### 并发与多实例（PR #9）

- **单实例部署**：compose 每个单元只有 1 个 token-service 副本；水平扩容不受支持
  （各副本内存快照互不可见）。扩展条件见运维手册 §1。
- 写路径有两道保护：
  - `keystore.json.lock`：持有期间保持文件句柄打开 + 内核级互斥锁（Unix `flock` /
    Windows 独占共享模式），锁文件记录 owner token（UUID v4）/ pid / 创建时间；
    持有者存活期间锁绝不会被回收，接管只能发生在内核锁已释放（前持有者退出/崩溃）
    时，5s 等待超时返回 `LockBusy`；
  - 写前磁盘指纹（SHA-256）比对：加载后被他人改动 → `Stale`，拒绝覆盖（`409`）。

### 运维注意事项

- token-service 是自身 JWKS 权威：rotate/retire/delete 会即时更新内存 JWKS cache 与
  issuer；其他服务经 `JWKSCache` 定时刷新（默认 1h）拉取新 JWKS，无需改动下游。
- `keytool inspect` 只输出 kid/status/时间戳，不会打印密钥材料。
- 日志只记录 kid 与状态迁移，绝不输出 PEM。
