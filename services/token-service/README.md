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
| POST | /api/tokens/resource | 获取 Resource Token |
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