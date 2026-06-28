# 44 — 权限变更 → 令牌失效（perm-epoch 双写，Phase 4.4）

**链路**：系统无服务端权限缓存（权限烤进 JWT）。权限变更（assign_permissions /
role 禁用·删除 / permission 禁用·删除）时：iam 同事务展开受影响 `user_ids` + 发
`iam.permission.changed`（outbox）+ **commit 后直写** Redis `superpowers:perm_epoch:{uid}=now`
（即时主路径）。gateway `auth_middleware` 比对 `token.iat < perm_epoch` → 401「permissions
changed」。gateway consumer 订阅事件用 **set-if-greater** 兜底补写（关闭直写 fail-open 窗口）。

> 失效判据：`iat < perm_epoch` 全序，任何「变更点之前签发」的 token 一律失效。perm_epoch key
> 两端前缀必须一致（`superpowers:`，compose 已统一）。

前置：`00-setup.md` 完成，会用到 0.4 的 JWT 解码。

## 场景 44-A：改角色权限 → 旧 token 被拦（核心主路径）

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| A1 | 准备一个属于某角色 `$RID` 的用户 `$UID`，让其登录拿到 `$OLDTOKEN`（这是「变更前」的 token） | 拿到旧 token | | |
| A2 | 解码确认 iat：`echo $OLDTOKEN \| cut -d. -f2 \| base64 -d \| jq .iat` | 记下 iat（秒） | | |
| A3 | 用 `$OLDTOKEN` 调一个需鉴权的接口：`curl -s -o /dev/null -w "%{http_code}" $BASE/v1/iam/users -H "Authorization: Bearer $OLDTOKEN"` | 200（变更前可用） | | |
| A4 | admin 改该角色权限（注意是 **PUT**）：<br>`curl -s -X PUT $BASE/v1/iam/roles/$RID/permissions -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' -d '{"permission_ids":["<某权限id>"]}'` | 200/204 | | |
| A5 | 查事件：`mysqlq "SELECT JSON_EXTRACT(envelope_json,'$.payload.reason'),JSON_EXTRACT(envelope_json,'$.payload.user_ids') FROM iam_outbox_events WHERE event_type='iam.permission.changed' ORDER BY id DESC LIMIT 1"` | reason=`role_permissions_changed`，user_ids 含 `$UID` | | |
| A6 | 查 Redis 戳：`redisq GET "superpowers:perm_epoch:$UID"` | 一个 unix 秒值，≥ A2 的 iat；`redisq TTL ...` >0 且 ≤900 | | |
| A7 | **再用 `$OLDTOKEN`** 调同一接口：`curl -s -o /dev/null -w "%{http_code}" $BASE/v1/iam/users -H "Authorization: Bearer $OLDTOKEN"` | **401**（body：permissions changed, re-authenticate） | | |
| A8 | 重新登录拿新 token，调同一接口 | 200（新 token 的 iat ≥ epoch，放行；且带新权限） | | |

## 场景 44-B：其它触发点

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| B1 | role 删除：`curl -X DELETE $BASE/v1/iam/roles/<rid> -H "Authorization: Bearer $TOKEN"` | 事件 reason=`role_deleted`；该角色下用户 perm_epoch 被写 | | |
| B2 | role 禁用：`authput`(PUT) `/v1/iam/roles/<rid>` body `{"status":"disabled"}` | reason=`role_disabled`（仅当从非 disabled→disabled 才触发） | | |
| B3 | permission 删除：`DELETE /v1/iam/permissions/<pid>` | reason=`permission_deleted`，user_ids 经 role_permissions JOIN 展开 | | |
| B4 | permission 禁用：`PUT /v1/iam/permissions/<pid>` body `{"status":"disabled"}` | reason=`permission_disabled` | | |
| B5 | 改名等无关更新（PUT role 只改 name） | **不**触发事件/戳（仅 status→disabled 才触发） | | |

> 校验 B5：改名后 `mysqlq` 查 outbox 最新行的 created_at 不应新增 permission.changed。

## 场景 44-C：gateway consumer 兜底（关闭 iam 直写 fail-open 窗口，⚠️ 破坏性）

模拟 iam 直写失败：让 iam 连不到 Redis，仅靠事件 → gateway consumer 补写 epoch。

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| C1 | 准备用户 `$UID2` 登录拿旧 token `$OLDTOKEN2` | | | |
| C2 | ⚠️ 让 iam 失去 Redis：最简方式停 Redis 会同时影响 gateway 读，故改为**只让 iam 直写失败但 gateway 仍可读**不易模拟；替代做法——清掉直写结果观察兜底：先正常触发变更，立刻 `redisq DEL "superpowers:perm_epoch:$UID2"`，再看 consumer 是否把它补回来 | | | |
| C3 | 触发权限变更（A4 同款，针对 $UID2 所在角色） | | | |
| C4 | 立刻 `redisq DEL "superpowers:perm_epoch:$UID2"`（模拟直写丢失） | | | |
| C5 | 等 1~3s（事件经 outbox→gateway consumer），再 `redisq GET "superpowers:perm_epoch:$UID2"` | 戳被 consumer 用事件 occurred_at **补回**（set-if-greater） | | |
| C6 | 用 `$OLDTOKEN2` 调鉴权接口 | 401（兜底生效） | | |

> 说明：C2 真要测「iam 直写失败」需断 iam→Redis 而保 gateway→Redis，环境上较难分离；用
> C4「删掉直写结果再看是否被补回」等价验证 consumer 兜底路径。

## 场景 44-D：fail-open（Redis 不可用时不锁死，⚠️ 破坏性）

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| D1 | ⚠️ `docker stop infrastructure-redis-1` | | | |
| D2 | 用任意有效 token 调鉴权接口 | 仍 200（gateway perm_epoch 读失败 → fail-open 放行；与 blacklist 一致） | | |
| D3 | `docker start infrastructure-redis-1` | 恢复 | | |

## 场景 44-E：单调性（set-if-greater 不回退）

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| E1 | 触发一次变更，记 `redisq GET superpowers:perm_epoch:$UID` = T1 | | | |
| E2 | 手动写一个更小的值：`redisq SET superpowers:perm_epoch:$UID 1`（直接 SET，绕过脚本） | 此为手工干预 | | |
| E3 | 再触发一次变更（iam 直写 / consumer 补写都走 set-if-greater） | epoch 变回 ≥T1 的较大值，不停留在 1（注：E2 的裸 SET 会被下一次 set-if-greater 纠正为更大值；验证写路径不回退） | | |

## 验收判定

- ✅ 通过：改权限后旧 token 立刻 401、Redis 戳被写（值≥iat、带 TTL）、重登可用；5 类触发点
  reason 正确、改名不误触发；consumer 能兜底补写；Redis 挂时 fail-open；set-if-greater 不回退。
- ❌ 失败常见原因：**两端 prefix 不一致**（iam 写 `superpowers:perm_epoch:` 但 gateway 读
  别的前缀 → 永远拦不住；检查 iam `REDIS_PREFIX` 与 gateway `GW_REDIS__PREFIX` 都为 `superpowers:`）；
  iam 没连上 Redis（看 iam 日志 `connected to redis for perm_epoch`，否则靠 consumer 兜底，有延迟）；
  gateway 未加 perm_epoch 比对（看 `auth_middleware`）。

## 排查命令

```bash
# 两端 prefix 是否一致（最常见坑）
docker compose -f infrastructure/docker-compose-services.yml exec api-gateway env | grep -i prefix
# gateway consumer 是否订阅
docker compose -f infrastructure/docker-compose-services.yml logs api-gateway | grep -i "perm_epoch consumer subscribed"
# iam 是否连上 Redis
docker compose -f infrastructure/docker-compose-services.yml logs iam-service | grep -i "redis for perm_epoch"
# 列出所有 perm_epoch 键
redisq KEYS "superpowers:perm_epoch:*"
```
