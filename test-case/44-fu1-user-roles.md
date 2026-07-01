# 44-fu1 · PUT /iam/users/:id/roles 手工验收

Phase 4.4-fu1 端到端验证：admin 改用户角色 → 该用户旧 token 立即失效 → 刷新拿到新权限。

## 前置

- iam-service / api-gateway / rabbitmq / redis / mysql 已起
- 有 admin 账号（default tenant）

## 步骤

1. **admin 登录**
   ```bash
   curl -s -X POST http://gateway/iam/auth/login -H 'Content-Type: application/json' \
     -d '{"username":"admin","password":"<pwd>","tenant_slug":"default"}' | jq
   ```
   记 `access_token` → `admin_tok`。

2. **建两个角色 r1 / r2**
   ```bash
   curl -s -X POST http://gateway/iam/roles -H "Authorization: Bearer $admin_tok" -H 'Content-Type: application/json' \
     -d '{"name":"r1","code":"r1","data_scope":0}'
   curl -s -X POST http://gateway/iam/roles -H "Authorization: Bearer $admin_tok" -H 'Content-Type: application/json' \
     -d '{"name":"r2","code":"r2","data_scope":0}'
   ```

3. **建 user u1 with role r1**
   ```bash
   curl -s -X POST http://gateway/iam/users -H "Authorization: Bearer $admin_tok" -H 'Content-Type: application/json' \
     -d '{"username":"u1","password":"P@ssw0rd!","nickname":"u1","role_ids":["<r1_id>"]}'
   ```

4. **u1 登录拿 token**
   ```bash
   curl -s -X POST http://gateway/iam/auth/login -H 'Content-Type: application/json' \
     -d '{"username":"u1","password":"P@ssw0rd!","tenant_slug":"default"}' | jq
   ```
   记 `access_token` → `u1_tok`。JWT payload 中 `roles` 应含 `r1`。

5. **admin 改 u1 → r2**
   ```bash
   curl -s -X PUT http://gateway/iam/users/<u1_id>/roles -H "Authorization: Bearer $admin_tok" -H 'Content-Type: application/json' \
     -d '{"role_ids":["<r2_id>"]}' | jq
   ```
   预期：200，body 中 `role_ids == ["<r2_id>"]`。

6. **u1 用旧 `u1_tok` 请求 → gateway 401**
   ```bash
   curl -s -o /dev/null -w '%{http_code}\n' http://gateway/iam/users/simple -H "Authorization: Bearer $u1_tok"
   ```
   预期：`401`。响应 body 含 `permissions changed, re-authenticate`。

7. **u1 刷新 → 拿到新 token**
   ```bash
   curl -s -X POST http://gateway/iam/auth/refresh -H 'Content-Type: application/json' \
     -d '{"refresh_token":"<u1_refresh>"}' | jq
   ```
   新 `access_token` JWT payload 中 `roles` 应为 `["r2"]`。

## 反向检查

- 调 `PUT /iam/users/<u1_id>/roles` body `{"role_ids":["<nonexistent>"]}` → 400，body 含 `"invalid role_ids"`。
- 调 `PUT /iam/users/ghost_id/roles` → 404。
- 重复调用同一 body（新旧集合相等）→ 200，但 gateway 401 不发生（无失效事件；用旧 token 请求仍正常，若 token 未被前一步失效过的话）。
