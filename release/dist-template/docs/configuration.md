# 配置参考

## .env 变量说明

### 镜像仓库

```env
REGISTRY=zhcoder-docker-registry.com:8000
IMAGE_TAG=v1.0.0
```

### 端口映射

```env
API_GATEWAY_PORT=3000    # 对外暴露的前端访问端口
FRONTEND_PORT=8080       # 前端 nginx 端口
```

### MySQL

```env
MYSQL_ROOT_PASSWORD=__CHANGE_ME__     # 必改：root 密码
MYSQL_HOST=mysql
MYSQL_PORT=3306
MYSQL_DATABASE=superpowers
MYSQL_USER=app
MYSQL_PASSWORD=__CHANGE_ME__         # 必改：应用密码
```

### Redis

```env
REDIS_PASSWORD=__CHANGE_ME__          # 必改
REDIS_PORT=6379
REDIS_DATABASES=16
REDIS_HOST=redis
```

### RabbitMQ

```env
RABBITMQ_DEFAULT_USER=guest
RABBITMQ_DEFAULT_PASS=__CHANGE_ME__   # 必改
RABBITMQ_PORT=5672
RABBITMQ_MANAGEMENT_PORT=15672
```

### Service Bootstrap Tokens

每个微服务启动时需要与 API 网关建立连接，bootstrap token 用于服务注册认证：

```env
IAM_DEV_BT=__CHANGE_ME__
TOKEN_DEV_BT=__CHANGE_ME__
NOTIFICATION_DEV_BT=__CHANGE_ME__
AUDIT_DEV_BT=__CHANGE_ME__
PRUE_DEV_BT=__CHANGE_ME__
GATEWAY_DEV_BT=__CHANGE_ME__
CONFIG_DEV_BT=__CHANGE_ME__
FILE_DEV_BT=__CHANGE_ME__
```

> 运行 `make secrets` 会自动生成所有 `__CHANGE_ME__` 值。

### 通用配置

```env
TENANT_ID=1
TOKEN_SERVICE_URL=http://token-service:8082
JWKS_URL=http://token-service:8082/.well-known/jwks.json
```

### Frontend 构建参数

```env
VITE_API_BASE=/api
VITE_PM_API_BASE=/pm-api
VITE_DISABLE_CAPTCHA=true
```

## 服务配置

各微服务的详细配置见 `config/` 目录：

| 路径 | 说明 |
|------|------|
| `config/api-gateway/api-gateway.toml` | 网关全局配置 |
| `config/api-gateway/routes/*.toml` | 路由规则 |
| `config/api-gateway/rate-limits.toml` | 限流规则 |
| `config/consul/connect.hcl` | Consul 连接配置 |
| `config/rabbitmq/` | RabbitMQ 配置 |
| `config/prometheus/prometheus.yml` | 监控目标 |
| `config/otel/collector.yaml` | OTel 收集器配置 |

## 生成新密钥

```bash
make secrets
```

此命令会：
1. 生成 RSA-2048 JWT 密钥对 → `secrets/jwt_private.pem`
2. 生成随机 MySQL/Redis/RabbitMQ 密码
3. 生成 8 个 service bootstrap token
4. 替换 `.env` 中的所有 `__CHANGE_ME__` 占位符
