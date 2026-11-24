# iam-service

统一身份与访问管理服务，整合认证、用户、角色、权限、部门、Token 管理能力。

## 功能模块

- **认证模块** (`/iam/auth/*`)
  - 登录/注销
  - Token 刷新
  - 密码修改

- **用户管理** (`/iam/users/*`)
  - 用户 CRUD
  - 用户角色分配

- **角色管理** (`/iam/roles/*`)
  - 角色 CRUD
  - 权限分配

- **权限管理** (`/iam/permissions/*`)
  - 权限 CRUD

- **部门管理** (`/iam/org-units/*`)
  - 部门树结构
  - 用户部门分配

- **会话管理** (`/iam/sessions/*`)
  - 会话查询
  - 强制注销

## 技术栈

- Rust 1.78+
- Axum 0.8 (Web 框架)
- SQLx (数据库)
- Redis (缓存)
- JWT (认证)
- Argon2 (密码加密)

## 配置

配置文件：`config/config.yaml`

```yaml
app:
  name: iam-service
  host: 0.0.0.0
  port: 3002

database:
  url: mysql://root:password@localhost:3306/iam_db
  max_connections: 20
  min_connections: 5

redis:
  url: redis://localhost:6379/0
  ttl_sec: 86400

jwt:
  private_key_path: keys/private.pem
  public_key_path: keys/public.pem
  access_token_ttl_minutes: 15
  refresh_token_ttl_days: 7

consul:
  host: localhost
  port: 8500
  service_name: iam-service
```

## 数据库迁移

- MySQL: `migrations/mysql/V001__init.sql`
- SQLite: `migrations/sqlite/V001__init.sql`

## 构建

```bash
cargo build --manifest-path services/iam-service/Cargo.toml
```

## 运行

```bash
cargo run --manifest-path services/iam-service/Cargo.toml
```

## Docker

```bash
docker build -t iam-service:latest services/iam-service/
docker run -p 3002:3002 iam-service:latest
```

### 容器内密码重置 CLI

管理员被锁死时的运维兜底（必须进入部署了 iam-service 的容器执行）：

```bash
docker exec -it <iam容器> iam_reset_password <username>          # 随机生成密码并打印（推荐）
docker exec -it <iam容器> iam_reset_password <username> -p <pw>  # 指定新密码（≥8 字符）
```


## 健康检查

- `/health` - 服务健康状态
- `/metrics` - Prometheus 指标端点
- `/swagger-ui` - Swagger UI 文档
- `/api-docs/openapi.json` - OpenAPI 规范

## 测试

```bash
cargo test --manifest-path services/iam-service/Cargo.toml
```

## 迁移说明

此服务合并了以下旧服务：
- `token-service` (Token 管理能力)
- `credential-login-service` (认证能力)
- `user-service` (用户、角色、权限管理)

旧服务端点迁移：
| 旧端点 | 新端点 |
|--------|--------|
| /api/v1/auth/* | /iam/auth/* |
| /api/v1/users/* | /iam/users/* |
| /api/v1/roles/* | /iam/roles/* |
| /api/v1/orgs/* | /iam/org-units/* |