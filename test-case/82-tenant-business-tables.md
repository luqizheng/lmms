# test-case/82-tenant-business-tables.md — Phase 8.2 iam 业务表 tenant 隔离

## 前置
- 本地起 `pnpm run dev:infra` + iam-service（`pnpm run dev`）
- 有 default tenant admin 账号

## 1. 注册两租户
```bash
curl -sX POST http://localhost:8080/iam/tenants -H 'Content-Type: application/json' \
  -d '{"tenant_slug":"acme","display_name":"Acme","admin_username":"admin","admin_password":"secret-acme-123","admin_email":"admin@acme.com"}'
# 记录 acme_tid
curl -sX POST http://localhost:8080/iam/tenants -H 'Content-Type: application/json' \
  -d '{"tenant_slug":"globex","display_name":"Globex","admin_username":"admin","admin_password":"secret-globex-123","admin_email":"admin@globex.com"}'
# 记录 globex_tid
```

## 2. 各自登录拿 token
```bash
ACME_TOKEN=$(curl -sX POST http://localhost:8080/iam/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"secret-acme-123","tenant_slug":"acme"}' | jq -r .access_token)

GLOBEX_TOKEN=$(curl -sX POST http://localhost:8080/iam/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"secret-globex-123","tenant_slug":"globex"}' | jq -r .access_token)
```

## 3. roles 隔离
```bash
# acme 建角色
curl -sX POST http://localhost:8080/iam/roles \
  -H "Authorization: Bearer $ACME_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"Acme Ops","code":"acme-ops","data_scope":0}' | jq -r .id
# 记录 acme_role_id

# acme list 可见
ACME_ROLES=$(curl -s http://localhost:8080/iam/roles -H "Authorization: Bearer $ACME_TOKEN")
echo "$ACME_ROLES" | jq '.[] | select(.code == "acme-ops")'  # 应找到

# globex list 不可见
GLOBEX_ROLES=$(curl -s http://localhost:8080/iam/roles -H "Authorization: Bearer $GLOBEX_TOKEN")
echo "$GLOBEX_ROLES" | jq '.[] | select(.code == "acme-ops")'  # 应为空
```

## 4. globex 靠 id 访问 acme 角色 → 404
```bash
curl -s -o /dev/null -w "%{http_code}" \
  http://localhost:8080/iam/roles/<acme_role_id> \
  -H "Authorization: Bearer $GLOBEX_TOKEN"
# 期望: 404
```

## 5. /iam/users 隔离
```bash
# acme 建用户
ACME_USER_ID=$(curl -sX POST http://localhost:8080/iam/users \
  -H "Authorization: Bearer $ACME_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"username":"acme-user-1","nickname":"Acme User 1","email":"user1@acme.com","password":"Test123456","role_ids":[]}' | jq -r .id)

# acme list 可见
curl -s http://localhost:8080/iam/users -H "Authorization: Bearer $ACME_TOKEN" | jq -r ".[] | select(.id == \"$ACME_USER_ID\")"  # 应找到

# globex list 不可见
curl -s http://localhost:8080/iam/users -H "Authorization: Bearer $GLOBEX_TOKEN" | jq -r ".[] | select(.id == \"$ACME_USER_ID\")"  # 应为空

# globex get by id → 404
curl -s -o /dev/null -w "%{http_code}" \
  "http://localhost:8080/iam/users/$ACME_USER_ID" \
  -H "Authorization: Bearer $GLOBEX_TOKEN"
# 期望: 404
```

## 6. org-units 隔离
```bash
# acme 建 org-unit
ACME_ORG_ID=$(curl -sX POST http://localhost:8080/iam/org-units \
  -H "Authorization: Bearer $ACME_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"Acme Engineering","code":"acme-eng","parent_id":null}' | jq -r .id)

# acme list 可见
curl -s http://localhost:8080/iam/org-units -H "Authorization: Bearer $ACME_TOKEN" | jq -r ".[] | select(.id == \"$ACME_ORG_ID\")"  # 应找到

# globex list 不可见
curl -s http://localhost:8080/iam/org-units -H "Authorization: Bearer $GLOBEX_TOKEN" | jq -r ".[] | select(.id == \"$ACME_ORG_ID\")"  # 应为空

# globex get by id → 404
curl -s -o /dev/null -w "%{http_code}" \
  "http://localhost:8080/iam/org-units/$ACME_ORG_ID" \
  -H "Authorization: Bearer $GLOBEX_TOKEN"
# 期望: 404
```

## 7. permissions 隔离（附加验证）
```bash
# acme 建 permission
ACME_PERM_ID=$(curl -sX POST http://localhost:8080/iam/permissions \
  -H "Authorization: Bearer $ACME_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"code":"acme:read","name":"Acme Read"}' | jq -r .id)

# acme list 可见
curl -s http://localhost:8080/iam/permissions -H "Authorization: Bearer $ACME_TOKEN" | jq -r ".[] | select(.code == \"acme:read\")"  # 应找到

# globex list 不可见
curl -s http://localhost:8080/iam/permissions -H "Authorization: Bearer $GLOBEX_TOKEN" | jq -r ".[] | select(.code == \"acme:read\")"  # 应为空
```

## 期望
所有跨租户访问返回 404 / 空列表；同租户 CRUD 正常。

## 验收标准
- [ ] acme 角色在 acme token 下可见，在 globex token 下不可见
- [ ] globex 无法通过 ID 访问 acme 角色（返回 404）
- [ ] acme 用户在 acme token 下可见，在 globex token 下不可见
- [ ] globex 无法通过 ID 访问 acme 用户（返回 404）
- [ ] acme 组织单元在 acme token 下可见，在 globex token 下不可见
- [ ] globex 无法通过 ID 访问 acme 组织单元（返回 404）
- [ ] acme 权限在 acme token 下可见，在 globex token 下不可见
