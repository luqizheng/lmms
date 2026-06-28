# 00 — 环境准备与通用工具

所有 Phase 4.x 手工场景的前置。完成本页后再执行 41~44。

## 0.1 起栈

```bash
cd infrastructure
docker compose up -d            # = docker-compose-infra.yml + docker-compose-services.yml
docker compose ps               # 等所有服务 healthy（mysql/redis/rabbitmq/consul + 各 service）
```

> 首次构建较慢。关注 `iam-service` / `notification-service` / `api-gateway` /
> `audit-service` / `iam-outbox-publisher` / `notification-outbox-publisher` 都起来。

| 步骤 | 操作 | 期望 | 实测 | P/F |
|------|------|------|------|-----|
| 0.1.1 | `docker compose up -d` | 全部容器 Up | | |
| 0.1.2 | `docker compose ps` | 所有 health=healthy；两个 outbox-publisher 在跑 | | |
| 0.1.3 | `curl -s localhost:3000/health` | `{"status":"ok"}` | | |
| 0.1.4 | 打开 `http://localhost:15672`（guest/guest） | 能进 RabbitMQ 管理台 | | |

## 0.2 准备 admin 账号 + 拿 token

首个注册用户会自动获得 admin 角色（见 `user_svc.rs` ensure_admin_role）。

```bash
export BASE=http://localhost:3000

# 查询是否需要初始化
curl -s $BASE/v1/iam/auth/setup-status

# 注册 admin（若 setup_required=true）
curl -s -X POST $BASE/v1/iam/auth/register \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"P@ssw0rd!","nickname":"Admin"}'

# 登录拿 token
export TOKEN=$(curl -s -X POST $BASE/v1/iam/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"P@ssw0rd!"}' | jq -r .access_token)
echo "$TOKEN"
```

| 步骤 | 操作 | 期望 | 实测 | P/F |
|------|------|------|------|-----|
| 0.2.1 | setup-status | 返回 `setup_required` 字段 | | |
| 0.2.2 | register admin | 200/201，返回用户 | | |
| 0.2.3 | login | 200，含 `access_token` | | |
| 0.2.4 | `echo $TOKEN` 非空 | JWT 字符串 | | |

## 0.3 通用工具命令（后续场景反复用）

```bash
# 鉴权请求
authget()  { curl -s "$BASE$1" -H "Authorization: Bearer $TOKEN"; }
authpost() { curl -s -X POST "$BASE$1" -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' -d "${2:-{}}"; }
authput()  { curl -s -X PUT  "$BASE$1" -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' -d "${2:-{}}"; }
authdel()  { curl -s -X DELETE "$BASE$1" -H "Authorization: Bearer $TOKEN"; }

# MySQL 查询
mysqlq() { docker exec -i infrastructure-mysql-1 mysql -uroot -prootpass superpowers -e "$1"; }

# Redis
redisq() { docker exec -i infrastructure-redis-1 redis-cli "$@"; }

# 看某服务日志（最近 50 行，跟随）
logs() { docker compose -f infrastructure/docker-compose-services.yml logs -f --tail=50 "$1"; }
```

> 容器名以实际 `docker compose ps` 输出为准（这里假设 compose project 名为 `infrastructure`）。
> 用 `docker ps --format '{{.Names}}'` 确认 mysql/redis 容器实际名。

## 0.4 解码 JWT（看 iat / permissions，4.4 要用）

```bash
# 取 JWT payload 段并 base64 解码
echo "$TOKEN" | cut -d. -f2 | base64 -d 2>/dev/null | jq .
# 关注：iat（签发秒）、permissions（权限数组）、sub（user_id）
```

| 步骤 | 操作 | 期望 | 实测 | P/F |
|------|------|------|------|-----|
| 0.4.1 | 解码 payload | 看到 `sub` / `iat` / `permissions` | | |
