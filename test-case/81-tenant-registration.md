# test-case/81-tenant-registration.md — Phase 8.1 多租户骨架

## 前置
- 起所有 dev 服务：`node scripts/dev-services.js`
- 已加载迁移 V003

## 步骤

### 1. 注册 acme 租户

```bash
curl -sX POST http://localhost:9000/iam/tenants \
  -H 'Content-Type: application/json' \
  -d '{"tenant_slug":"acme","display_name":"Acme Corp","admin_username":"admin","admin_password":"secret-acme-123","admin_email":"admin@acme.com"}'
```

期望：`201 { "tenant_id": <id>, "admin_user_id": "<uuid>" }`，记录 acme_tid。

### 2. 注册 globex 租户（同样 admin 用户名）

```bash
curl -sX POST http://localhost:9000/iam/tenants \
  -H 'Content-Type: application/json' \
  -d '{"tenant_slug":"globex","display_name":"Globex","admin_username":"admin","admin_password":"secret-globex-123"}'
```

期望：`201`（unique 是 `(tenant_id, username)`，不冲突）。

### 3. slug 冲突

```bash
curl -sX POST http://localhost:9000/iam/tenants \
  -H 'Content-Type: application/json' \
  -d '{"tenant_slug":"acme","display_name":"Dup","admin_username":"x","admin_password":"secret-xx-12"}'
```

期望：`409`。

### 4. 登录 acme.admin

```bash
curl -sX POST http://localhost:9000/iam/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"secret-acme-123","tenant_slug":"acme"}'
```

期望：`200`，解 JWT payload 应见 `"tid": <acme_tid>`。

### 5. iam_users 隔离（本期唯一隔离表）

```bash
# 用 acme admin token 调 /iam/users
curl -H "Authorization: Bearer <acme_jwt>" http://localhost:9000/iam/users
```

期望：仅 acme admin（不见 globex admin、不见 default tenant 老用户）。

⚠️ 注：本期 `/iam/users` SELECT 尚未注入 `tenant_id` 过滤（属 Phase 8.2 改造），
故步骤 5 在 8.1 当前提交下仍可能看到跨租户用户。这是已知限制，spec §5 / §6 明确划归 Phase 8.2 / 8.3。

### 6. 限制：roles / audit / announcements 仍跨租户可见

⚠️ Phase 8.1 仅 iam_users 加了 tenant_id，roles/permissions/announcements/audit_logs 等表 schema 不动。
跨租户访问这些资源仍然可见，将由 Phase 8.2 表迁移 + Phase 8.3 全面贯通解决。
本期手工验收不为这些表设期望。
