# token-service

OAuth 2.0 Token 服务，负责 Access Token、Refresh Token、API Key 的颁发与管理。

## 功能模块

- **Access Token** (`/api/tokens/access`) - 获取访问令牌
- **Refresh Token** (`/api/tokens/refresh`) - 刷新令牌
- **Revoke** (`/api/tokens/revoke`) - 撤销令牌
- **Resource Token** (`/api/tokens/resource`) - 获取资源令牌
- **JWKS** (`/.well-known/jwks.json`) - JSON Web Key Set
- **API Key** (`/api/api-keys`) - 创建 API Key
- **User JWT** (`/api/v1/tokens/user-jwt`) - 用户 JWT

## 技术栈

- Rust 1.78+
- Axum 0.8 (Web 框架)
- SQLx (数据库)
- JWT-Simple (JWT 处理)
- Consul (服务发现)

## 配置

配置文件：`config/config.yaml`

```yaml
server:
  host: 0.0.0.0
  port: 4000

issuer: superpowers-token-server

database:
  db_type: mysql
  url: mysql://root:password@localhost:3306/token_db

jwt_private_key_path: keys/private.pem
jwt_public_key_path: keys/public.pem

default_resource_ttl_sec: 3600

user_service_url: http://iam-service:3002

consul:
  addr: http://localhost:8500
  service_name: token-service
  service_id_file: /tmp/token-service.id
```

## 数据库迁移

迁移文件：`migrations/20260611000001_init.sql`

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

## 端点

- `GET /health` - 健康检查
- `GET /metrics` - Prometheus 指标端点
- `GET /swagger-ui` - Swagger UI 文档
- `GET /api-docs/openapi.json` - OpenAPI 规范
- `POST /api/tokens/access` - 获取 Access Token
- `POST /api/tokens/refresh` - 刷新 Token
- `POST /api/tokens/revoke` - 撤销 Token
- `POST /api/tokens/resource` - 获取 Resource Token
- `GET /api/tokens/.well-known/jwks.json` - JWKS
- `GET /.well-known/jwks.json` - JWKS (别名)
- `POST /api/api-keys` - 创建 API Key
- `POST /api/v1/tokens/user-jwt` - 获取用户 JWT

## 测试

```bash
cargo test --manifest-path services/token-service/Cargo.toml
```