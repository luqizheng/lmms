# test-case/83-cross-tenant.md — Phase 8.3 跨租户黑名单

## 前置
- 本地起 `pnpm run dev:infra` + iam-service（`pnpm run dev`）
- admin 已分配 platform_admin 角色

## 1. 给 admin 分配 platform_admin 角色
```bash
ADMIN_TOKEN=$(curl -sX POST http://localhost:8080/iam/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"<admin-password>"}' | jq -r .access_token)

ADMIN_ID=$(curl -s http://localhost:8080/iam/users?keyword=admin \
  -H "Authorization: Bearer $ADMIN_TOKEN" | jq -r '.items[0].id')

curl -sX PUT "http://localhost:8080/iam/users/$ADMIN_ID/roles" \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"role_ids":["role-platform-admin-seed"]}'

# 重新登录获取含 platform_admin 角色的 token
PA_TOKEN=$(curl -sX POST http://localhost:8080/iam/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"<admin-password>"}' | jq -r .access_token)
```

## 2. 注册两租户
```bash
curl -sX POST http://localhost:8080/iam/tenants -H 'Content-Type: application/json' \
  -d '{"tenant_slug":"acme","display_name":"Acme","admin_username":"admin","admin_password":"secret-acme-123","admin_email":"admin@acme.com"}'

curl -sX POST http://localhost:8080/iam/tenants -H 'Content-Type: application/json' \
  -d '{"tenant_slug":"globex","display_name":"Globex","admin_username":"admin","admin_password":"secret-globex-123","admin_email":"admin@globex.com"}'
```

## 3. 各自登录拿 token
```bash
ACME_TOKEN=$(curl -sX POST http://localhost:8080/iam/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"secret-acme-123","tenant_slug":"acme"}' | jq -r .access_token)

GLOBEX_TOKEN=$(curl -sX POST http://localhost:8080/iam/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"secret-globex-123","tenant_slug":"globex"}' | jq -r .access_token)
```

## 4. globex token → acme 角色 → 404（跨租户黑名单）
```bash
# acme 建角色
ACME_ROLE_ID=$(curl -sX POST http://localhost:8080/iam/roles \
  -H "Authorization: Bearer $ACME_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"Acme Ops","code":"acme-ops","data_scope":3}' | jq -r .id)

# globex token 访问 acme 角色 → 404
curl -s -o /dev/null -w "%{http_code}" \
  "http://localhost:8080/iam/roles/$ACME_ROLE_ID" \
  -H "Authorization: Bearer $GLOBEX_TOKEN"
# 期望: 404
```

## 5. globex token → acme 用户 → 404
```bash
# acme 建用户
ACME_USER_ID=$(curl -sX POST http://localhost:8080/iam/users \
  -H "Authorization: Bearer $ACME_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"username":"acme-user-1","nickname":"Acme User 1","email":"user1@acme.com","password":"Test123456","role_ids":[]}' | jq -r .id)

# globex token 访问 acme 用户 → 404
curl -s -o /dev/null -w "%{http_code}" \
  "http://localhost:8080/iam/users/$ACME_USER_ID" \
  -H "Authorization: Bearer $GLOBEX_TOKEN"
# 期望: 404
```

## 6. globex token → acme 组织单元 → 404
```bash
ACME_ORG_ID=$(curl -sX POST http://localhost:8080/iam/org-units \
  -H "Authorization: Bearer $ACME_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"Acme Engineering","code":"acme-eng","parent_id":null}' | jq -r .id)

curl -s -o /dev/null -w "%{http_code}" \
  "http://localhost:8080/iam/org-units/$ACME_ORG_ID" \
  -H "Authorization: Bearer $GLOBEX_TOKEN"
# 期望: 404
```

## 7. globex token → acme 权限 → 不可见
```bash
ACME_PERM_ID=$(curl -sX POST http://localhost:8080/iam/permissions \
  -H "Authorization: Bearer $ACME_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"code":"acme:read","name":"Acme Read"}' | jq -r .id)

# acme list 可见
curl -s http://localhost:8080/iam/permissions -H "Authorization: Bearer $ACME_TOKEN" | jq -r ".[] | select(.code == \"acme:read\")"  # 应找到

# globex list 不可见
curl -s http://localhost:8080/iam/permissions -H "Authorization: Bearer $GLOBEX_TOKEN" | jq -r ".[] | select(.code == \"acme:read\")"  # 应为空
```

## 8. platform_admin 跨租户可见
```bash
# platform_admin 可查看所有租户用户（§3.6）
curl -s http://localhost:8080/iam/users \
  -H "Authorization: Bearer $PA_TOKEN" | jq '.total'
# 期望: total > 1（能看到多租户用户）

# platform_admin 可查看任何租户角色
curl -s "http://localhost:8080/iam/roles/$ACME_ROLE_ID" \
  -H "Authorization: Bearer $PA_TOKEN"
# 期望: 200, 返回 acme 角色详情
```

## 9. JWT 缺 tid → 401
```bash
# 构造不含 tid 的 JWT，访问受保护接口
# (由 testcontainers 测试 tid_missing_returns_error 覆盖)
```

## 10. get_claims 租户隔离
```bash
# internal 端点按 user_id + tenant_id 查询
# acme 用户在 globex 租户下查询返回 UserNotFound
# (由 testcontainers 测试 get_claims_wrong_tenant 覆盖)
```

## 验收标准
- [ ] globex 无法通过 ID 访问 acme 角色（返回 404）
- [ ] globex 无法通过 ID 访问 acme 用户（返回 404）
- [ ] globex 无法通过 ID 访问 acme 组织单元（返回 404）
- [ ] globex 无法查看 acme 权限列表
- [ ] platform_admin 可跨租户查看所有用户、角色、组织单元
- [ ] JWT 缺 tid claim 时返回 MissingTenantId 错误
- [ ] get_claims 错误租户返回 UserNotFound
