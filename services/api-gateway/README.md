# api-gateway

API 网关服务，Rust + Axum，负责路由、JWT 校验、Redis 黑名单、Consul 服务发现。

## 目录结构

```
services/api-gateway/
├── Cargo.toml
├── Dockerfile
├── src/
│   ├── main.rs           # 入口、CLI、proxy_handler、SIGHUP/admin reload
│   ├── lib.rs            # 模块聚合、AppState、create_app
│   ├── cli.rs            # clap 子命令：validate / serve
│   ├── config.rs         # GatewayConfig / RouteConfig / RewriteSpec
│   ├── routes_loader.rs  # 加载 *.router.toml
│   ├── router.rs         # 转发到后端服务
│   ├── discovery.rs      # Consul 服务发现
│   ├── jwt.rs / jwks.rs  # JWT 校验、JWKS 拉取
│   ├── blacklist.rs      # Redis 黑名单
│   ├── middleware/       # auth_middleware 等
│   └── rewrite.rs        # rewrite 链
└── tests/                # 集成测试
```

## CLI

```powershell
# 校验路由（不改配置）
cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes

# 启动网关
cargo run -p api-gateway -- serve `
  --routes docker\config\api-gateway\routes `
  --admin-port 9090
```

`admin-port = 0` 关闭 admin 端口。POSIX 下还监听 `SIGHUP` 热重载路由。

## 全局配置文件

`main.rs` 用 `config` crate 从以下位置加载全局配置：

| 优先级 | 来源 | 说明 |
|---|---|---|
| 1 | `config/api-gateway.{toml,json,yaml,...}` | 本地文件，相对工作目录 |
| 2 | `GW_*` 环境变量 | 见下表 |

### 文件路径

- 默认相对路径：`config/api-gateway`（不带后缀）
- 实际配置文件在仓库内的 `docker/config/api-gateway/api-gateway.toml`
- 容器内通常挂载到 `config/api-gateway/`

### 环境变量

前缀 `GW`，分隔符 `__`（双下划线进入嵌套段，单下划线进入字段）：

| 环境变量 | 对应字段 | 默认值 |
|---|---|---|
| `GW_SERVER__HOST` | `server.host` | `0.0.0.0` |
| `GW_SERVER__PORT` | `server.port` | `3000` |
| `GW_REDIS__URL` | `redis.url` | `redis://127.0.0.1:6379` |
| `GW_REDIS__PREFIX` | `redis.prefix` | `superpowers:` |
| `GW_CONSUL_ADDR` | `consul_addr` | `http://127.0.0.1:8500` |
| `GW_JWKS_URL` | `jwks_url` | `http://127.0.0.1:4000/.well-known/jwks.json` |
| `GW_ISSUER` | `issuer` | `superpowers-token-server` |
| `GW_ROUTES` | `routes` | （由 `--routes` 参数覆盖） |

其他常用环境变量：

| 变量 | 用途 |
|---|---|
| `RUST_LOG` | tracing 日志级别，默认 `api_gateway=debug,info` |

### 配置示例

`config/api-gateway.toml`：

```toml
[server]
host = "0.0.0.0"
port = 3000

[redis]
url = "redis://localhost:6379"
prefix = "superpowers_"

consul_addr = "http://localhost:8500"
jwks_url = "http://localhost:4000/.well-known/jwks.json"
issuer = "superpowers-token-server"
```

## 路由文件

`--routes` 指定目录，扫描 `*.router.toml` 按文件名排序加载，重复 `match` 启动失败。

`docker/config/api-gateway/routes/` 示例：

- `auth.router.toml`
- `token.router.toml`
- `user.router.toml`
- `roles.router.toml`
- `permissions.router.toml`
- `notification.router.toml`

### Route 字段

| 字段 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `match` | string | 是 | URL 模式，匹配 `/api<match>`，`**` 表示前缀通配 |
| `service` | string | 是 | Consul 注册的服务名 |
| `public` | bool | 否 | `true` 跳过 JWT 校验，默认 `false` |
| `rewrite` | array | 否 | rewrite 链，按顺序执行 |

### Rewrite 类型

| `type` | 字段 | 用途 |
|---|---|---|
| `strip_prefix` | `prefix` | 去掉请求路径前缀 |
| `redirect` | `status`, `to` | 重定向 |
| `header_set` | `name`, `value` | 覆盖请求头 |
| `header_add` | `name`, `value` | 追加请求头 |
| `path_param` | `from`, `to` | 路径段改写 |

### 路由示例

```toml
[[route]]
match = "/api/v1/users/**"
service = "user-service"
public = false

[[route]]
match = "/api/v1/auth/logout"
service = "auth-service"
public = true
rewrite = [
  { type = "header_set", name = "X-Auth-Audit", value = "logout" },
  { type = "strip_prefix", prefix = "/api" },
]
```

## 热重载

两种方式，运行时无需重启：

- `POST http://<host>:<admin-port>/__admin/reload`
- POSIX `kill -HUP <pid>`

## 端点

- `GET /health` — 健康检查
- `/*`（嵌套在 `/api`） — 反向代理入口
- `POST /__admin/reload`（`admin-port`，仅 `serve` 子命令启用）

## 约束

- 沙箱内不可执行 `cargo build/test`，需在带 `vcvarsall x64` 的 PowerShell 跑
- 不要回退 `aws_lc_rs` 到 `ring`
- 改 `routes/*.router.toml` 后必须先 `validate` 再提交

## docker-compose 示例

依赖：Consul、Redis（黑名单）、token-service（JWKS）。下面片段接在 `infrastructure/docker-compose.yml` 的服务列表后，复用 `superpowers` 网络。

`services/api-gateway/docker-compose.yaml`：

```yaml
services:
  api-gateway:
    build:
      context: ../..                # 仓库根，含 Cargo.toml
      dockerfile: services/api-gateway/Dockerfile
    image: api-gateway:local
    container_name: api-gateway
    depends_on:
      consul:
        condition: service_healthy
      redis:
        condition: service_healthy
      token-service:
        condition: service_started
    environment:
      GW_SERVER__HOST: "0.0.0.0"
      GW_SERVER__PORT: "3000"
      GW_REDIS__URL: "redis://redis:6379"
      GW_REDIS__PREFIX: "superpowers_"
      GW_CONSUL_ADDR: "http://consul:8500"
      GW_JWKS_URL: "http://token-service:4000/.well-known/jwks.json"
      GW_ISSUER: "superpowers-token-server"
      RUST_LOG: "api_gateway=debug,info"
    command:
      - "serve"
      - "--routes"
      - "/app/config/routes"
      - "--admin-port"
      - "9090"
    volumes:
      - ../../docker/config/api-gateway/routes:/app/config/routes:ro
    ports:
      - "3000:3000"   # 业务端口
      - "9090:9090"   # admin reload
    networks:
      - superpowers
    restart: unless-stopped

  token-service:
    # 示例：实际镜像与端口以仓库 token-service 为准
    image: token-service:local
    container_name: token-service
    depends_on:
      redis:
        condition: service_healthy
    environment:
      SERVER__PORT: "4000"
      REDIS__URL: "redis://redis:6379"
    ports:
      - "4000:4000"
    networks:
      - superpowers
```

启动与运维：

```powershell
# 启动基础设施（consul/redis/mysql）
docker compose -f infrastructure\docker-compose.yml up -d

# 启动网关
docker compose -f services\api-gateway\docker-compose.yaml up -d --build

# 校验路由
docker exec api-gateway /app/api-gateway validate --routes /app/config/routes

# 热重载路由
curl -X POST http://localhost:9090/__admin/reload
```
