# Secret Rotation SOP — 密钥轮换标准操作流程

> **状态**：草案 · 待评审
> **关联**：TODO.md §Phase 10 `密钥轮换机制 SOP（JWT 私钥、DB 密码、API key）`
> **适用范围**：coder-micro-services 平台所有 secret 类资产

---

## 0. 一句话

> 当前项目 secret 资产分三类：JWT 私钥（静态 PEM）、DB 密码（`.env` 环境变量）、Redis 密码（`.env`）。
> **没有任何自动 rotation 机制**。本文档给出三类 secret 的轮换 SOP（含手动应急流程与未来自动化路线图）。

---

## 1. 资产盘点

| Secret 类型 | 当前位置 | 引用方 | Rotation 状态 |
|-------------|----------|--------|---------------|
| JWT 私钥 (RS256) | `/app/keys/private.pem` (容器) / `keys/private.pem` (host) | iam-service (签发 user-jwt) · token-service (签发 service-jwt) | ❌ 无 rotation · 单 PEM |
| JWT 公钥 | `/app/keys/public.pem` | 7 个下游服务 (JWKS cache) | ✅ 已 JWKS 化 (Phase 5B) |
| MySQL root 密码 | `MYSQL_ROOT_PASSWORD` (`.env`) | infra MySQL 容器 + 7 个服务的 `database.url` | ❌ 无 rotation · dev 环境硬编码 `root:rootpass` |
| Redis 密码 | `REDIS_PASSWORD` (`.env`) | infra Redis 容器 + api-gateway (限流) + notification (限流) | ❌ 无 rotation |
| Consul ACL token | 无（dev 单节点，无 ACL） | registrator + 各服务 | N/A |
| API key | 无（项目未引入外部 API key） | — | N/A |

### 1.1 当前架构图

```
iam-service
   ├─ private_key_path: /app/keys/private.pem (静态)
   └─ public_key_path:  /app/keys/public.pem  (静态)
            ↓ (Phase 5B JWKS 化)
common-jwt::JWKSCache (其他 7 服务)
   └─ url: http://iam-service:8081/.well-known/jwks.json
```

---

## 2. JWT 私钥轮换（重点）

### 2.1 当前痛点

- iam-service 用单 PEM 签 user-jwt；token-service 用同 PEM 签 service-jwt
- rotation = 重启所有服务 + 短期断签（用户体验断）
- 已签发 token 在 rotation 后仍可用旧公钥验签（直到 exp），需要 graceful drain

### 2.2 目标架构（中期路线）

```
iam-service 启动时：
  ├─ 读 private_key_path + private_key_path.next (kid=N+1, 待启用)
  ├─ 两者都加载到内存
  ├─ 默认签发用 kid=N+1（新 key）
  └─ 暴露 JWKS:
       GET /.well-known/jwks.json
       {
         "keys": [
           { "kid": "N",   "kty": "RSA", ... },  // 老 key, 仍接受验签
           { "kid": "N+1", "kty": "RSA", ... }   // 新 key, 用于新签发
         ]
       }
```

- **多 key 加载**：`libs/common-jwt::JWKSCache` 扩展为支持多 kid（Phase 5B 已是单 key）
- **灰度签发**：config 配 `signing_kid = "N+1"`，运行时可热切换
- **老 key 下线**：等所有老 token exp 后（access_token_ttl_minutes=15）从 JWKS 移除 kid=N

### 2.3 手动 SOP（当前可执行）

**频率**：建议每 90 天一次；合规事件 / 泄漏事件发生后立即执行。

**前置**：
- 确认所有客户端已升级到支持 JWKS 的版本（Phase 5B 已全部支持）
- 选定维护窗口（建议凌晨低峰）

**步骤**：

```bash
# 1. 生成新 key pair (kid=N+1)
cd keys/
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out private.pem.new
openssl rsa -in private.pem.new -pubout -out public.pem.new
chmod 600 private.pem.new public.pem.new

# 2. 原子替换 (先 mv 备份,再切)
cp private.pem private.pem.bak.$(date +%Y%m%d)
mv private.pem.new private.pem
cp public.pem public.pem.bak.$(date +%Y%m%d)
mv public.pem.new public.pem

# 3. 滚动重启 (灰度,确保不同时全切)
# 3.1 先重启 iam-service
docker compose -f infrastructure/docker-compose-services.yml restart iam-service
# 3.2 等 30s,确认健康
curl -fs http://localhost:8081/health
# 3.3 验证新 JWKS 暴露 (iam-service 自身要带 kid 区分 — 当前实现无 kid,见 §2.4 已知缺陷)
curl -fs http://localhost:8081/.well-known/jwks.json | jq .

# 3.4 依次重启 token-service, api-gateway, 各下游服务
#    (Phase 5B 后, 下游从 iam-service 拉 JWKS,会自动拿到新 key)
docker compose -f infrastructure/docker-compose-services.yml restart token-service api-gateway \
    notification-service audit-service config-service file-service prue_backend

# 3.5 等待 access_token_ttl_minutes (默认 15 min) 让老 token 自然过期

# 3.6 删除备份 (逾期后)
rm -f keys/*.bak.*

# 4. 验证
# 4.1 新登录应得新签名的 token (用 token-service 的 inspect 端点验签)
# 4.2 老 token 在 15 min 内仍可用 (graceful)
```

### 2.4 已知缺陷 / TODO

- **无 kid 注入**：当前 iam-service 签发的 JWT 无 `kid` header claim。下游 JWKS cache 当前匹配逻辑只能验单 key。
- **JWKS endpoint 实现位置**：`/.well-known/jwks.json` 当前**未实现**。Phase 5B spec §3.4 提到要加，但 follow-up 未做。
- **建议**：把"实现 kid + JWKS endpoint"作为 Phase 5-B-fu7 spec 立项。否则手动 SOP §2.3 在多 kid 并存场景下不可靠。

---

## 3. DB 密码轮换

### 3.1 当前问题

```yaml
# services/iam-service/config/config.yaml
database:
  url: mysql://root:rootpass@mysql:3306/superpowers?ssl-mode=DISABLED
```

`root:rootpass` **硬编码**。生产部署必须替换，但流程未文档化。

### 3.2 手动 SOP

**频率**：建议每 180 天一次；人员异动后立即执行。

**步骤**：

```bash
# 1. 生成新密码 (16+ 字符, 含大小写+数字+符号)
NEW_PASS=$(openssl rand -base64 24 | tr -d '/+=' | head -c 24)
echo "新密码: $NEW_PASS"   # 仅显示一次,记录到密码管理器

# 2. MySQL 内轮换 root 密码
docker exec -it mysql mysql -uroot -p"$OLD_PASS" -e \
  "ALTER USER 'root'@'%' IDENTIFIED BY '$NEW_PASS'; FLUSH PRIVILEGES;"

# 3. 更新 secrets 存储
# 3.1 .env (dev/staging)
sed -i "s/^MYSQL_ROOT_PASSWORD=.*/MYSQL_ROOT_PASSWORD=$NEW_PASS/" infrastructure/.env

# 3.2 production secrets manager (未来)
# vault kv put secret/coder-micro-services/mysql/root password="$NEW_PASS"

# 4. 滚动重启所有服务 (让所有 pool 重新建连)
docker compose -f infrastructure/docker-compose-services.yml restart \
    iam-service notification-service audit-service config-service \
    file-service token-service

# 5. 验证
curl -fs http://localhost:8081/health  # iam-service
curl -fs http://localhost:8083/health  # notification-service
# ... 逐服务

# 6. 旧密码作废 (确认新密码生效后,从密码管理器删除旧密码条目)
```

### 3.3 改进路线（中期）

- 所有 `config/config.yaml` 中的 `database.url` 改为读 env var：
  ```yaml
  database:
    url: ${DATABASE_URL}
  ```
- `common-config` 统一从环境变量 / vault 解析 secret
- CI 闸门：`scripts/check-config-secrets.js` 检测 config.yaml 中是否含明文密码

---

## 4. Redis 密码轮换

### 4.1 手动 SOP

**频率**：每 180 天 / 异动后。

**步骤**：

```bash
# 1. 生成新密码
NEW_REDIS_PASS=$(openssl rand -base64 24 | tr -d '/+=' | head -c 24)

# 2. Redis 内轮换 (假设已有 ACL,否则用 requirepass 模式)
docker exec -it redis redis-cli -a "$OLD_REDIS_PASS" CONFIG SET requirepass "$NEW_REDIS_PASS"
docker exec -it redis redis-cli -a "$NEW_REDIS_PASS" CONFIG REWRITE

# 3. 更新 .env
sed -i "s/^REDIS_PASSWORD=.*/REDIS_PASSWORD=$NEW_REDIS_PASS/" infrastructure/.env

# 4. 滚动重启所有读 Redis 的服务 (api-gateway 限流 + notification 限流 + iam perm_epoch + 任何 common-ratelimit 用户)
docker compose -f infrastructure/docker-compose-services.yml restart \
    api-gateway notification-service iam-service

# 5. 验证
curl -fs http://localhost:3000/health
```

### 4.2 Redis ACL 模式（推荐）

dev 单节点用 requirepass 即可；prod 推荐切 Redis 6+ ACL：
- 创建专用 user `coder_app` with `@read @write @connection ~* &* +@all`
- 密码 per-user,rotation 不影响其他 user

---

## 5. Consul ACL Token（未来项）

当前 dev 无 ACL,prod 启用后需：
- token 走 Vault dynamic secret
- `registrator` / 各服务通过 env 注入
- rotation = `consul acl token update`

**当前 N/A**,留待 prod 部署时建 spec。

---

## 6. 通用原则

1. **零明文**：所有 secret 不进 git。`.env` 必须 `.gitignore`（已 ✅）。
2. **零共享**：每个 secret 一个 owner。共享 = 等同无 owner。
3. **可轮换**：每 secret 必须能在不停服下轮换（JWT 需 kid + JWKS 配合）。
4. **可审计**：rotation 事件记录到 `docs/runbooks/rotation-log.md`（每次执行追加一行）。
5. **应急流程**：泄漏 → 立即 rotation + 通知 + 复盘 → 写 `docs/runbooks/incidents/YYYY-MM-DD-<topic>.md`。

---

## 7. 自动化路线图（Roadmap 2.0 候选）

| 阶段 | 目标 | 触发 |
|------|------|------|
| Phase R2-S1 | kid + JWKS endpoint 实现（iam-service） | Phase 5-B-fu7 spec |
| Phase R2-S2 | `common-config` 统一从 env / vault 解析 secret | prod 部署准备 |
| Phase R2-S3 | Vault 集成 + dynamic secret for MySQL / Redis | 合规需求触发 |
| Phase R2-S4 | rotation cron + 自动验证 + Slack 通知 | SOP 稳定后 |

---

## 8. 关联

- `docs/superpowers/specs/2026-07-07-phase5b-jwt-signature-and-service-jwt-design.md` §0.5.3 #3 "生产应拆分 user/service key"
- `services/iam-service/config/config.yaml` — `root:rootpass` 硬编码（待改）
- `infrastructure/.env` — DB / Redis 密码
- `keys/private.pem` · `keys/public.pem` — 静态 PEM
- TODO.md §Phase 10 `密钥轮换机制 SOP`