# Docker 正式发布部署

> 适用：生产 / 预发布环境，基于 `docker compose` 一键拉起 coder-micro-services 全栈。

---

## 1. 目录结构

```
docker/
├── docker-compose.yml          # 编排入口
├── config/                     # 各服务运行配置（已挂载进容器）
│   ├── api-gateway/            # 网关主配置 + routes/*.router.toml
│   ├── token-service.toml      # 令牌服务配置
│   ├── token-service/          # 令牌服务子配置（clients.json）
│   ├── notification-service/   # 通知服务配置
│   ├── consul/                 # Consul 服务发现配置
│   └── nginx/nginx.conf        # 反向代理
├── secrets/                    # RSA 密钥对（不入仓，部署时注入）
│   ├── private.pem
│   └── public.pem
├── mysql/data/                 # MySQL 数据卷（首次启动初始化）
└── redis_data/                 # Redis 数据卷
```

---

## 2. 前置条件

| 项 | 要求 |
|---|---|
| Docker Engine | ≥ 24.0 |
| Docker Compose | v2（`docker compose` 插件） |
| 主机资源 | CPU ≥ 4 核，内存 ≥ 8 GB，磁盘 ≥ 50 GB |
| 网络 | 可访问 `zhcoder-docker-registry.com:8000` 私有镜像仓库 |
| 端口 | 80、3000、3306、4000、5000、6000、6379、8500 |

---

## 3. 密钥准备

仓库不包含私钥，部署前手动生成并放入 `secrets/`：

```powershell
# 生成 RSA 4096 密钥对
openssl genpkey -algorithm RSA -out secrets\private.pem -pkeyopt rsa_keygen_bits:4096
openssl rsa -in secrets\private.pem -pubout -out secrets\public.pem

# 收紧权限（Linux 主机必做）
icacls secrets\private.pem /inheritance:r /grant:r "$($env:USERNAME):(R)"
```

> ⚠️ 私钥一旦泄露须立即全量轮换（`private.pem` + 所有签发中的 JWT 失效）。

---

## 4. 部署步骤

```powershell
# 4.1 进入部署目录
cd d:\projects\coder-micro-services\docker

# 4.2 拉取镜像（网络异常时加 --pull never 走本地缓存）
docker compose pull

# 4.3 后台启动全栈
docker compose up -d

# 4.4 查看启动状态
docker compose ps

# 4.5 跟踪日志（按服务名）
docker compose logs -f token-server
docker compose logs -f api-gateway
```

启动顺序由 `depends_on.condition: service_healthy` 控制：
`mysql / redis / consul` → `token-server` → `api-gateway` → `auth-service / user-service` → `nginx`。

---

## 5. 端口与访问

| 服务 | 容器内 | 宿主机 | 用途 |
|---|---|---|---|
| nginx | 80 | 80 | 前端静态资源 + 反向代理 |
| api-gateway | 3000 | 3000 | 对外 API 入口 |
| token-server | 4000 | 4000 | OAuth 2.0 / JWKS |
| auth-service | 5000 | 5000 | 登录认证 |
| user-service | 6000 | 6000 | 用户 / 角色 / 权限 |
| mysql | 3306 | 3306 | 数据库（仅内网开放） |
| redis | 6379 | 6379 | 缓存 / Token 存储 |
| consul | 8500 | 8500 | 服务发现 UI（仅内网开放） |

健康检查入口：
- `http://<host>:3000/health`
- `http://<host>:4000/health`
- `http://<host>:4000/.well-known/jwks.json`
- `http://<host>:8500/ui`

---

## 6. 配置变更

### 6.1 路由变更

修改 `config/api-gateway/routes/*.router.toml` 后：

```powershell
cd d:\projects\coder-micro-services
cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes
# 校验通过后调用网关热重载
curl -X POST http://<host>:3000/__admin/reload
```

### 6.2 服务配置

`config/*.toml` / `config/*/config.yaml` 通过卷挂载进容器，**改完需重启对应服务**：

```powershell
docker compose restart api-gateway
```

### 6.3 环境变量

`docker-compose.yml` 中 `environment` 段，使用 `__` 表示 TOML 嵌套键（如 `TS_SERVER__PORT`）。
修改后必须 `docker compose up -d` 重建容器。

---

## 7. 数据持久化与备份

| 卷 | 路径 | 备份建议 |
|---|---|---|
| MySQL | `mysql/data/` | 每日 `mysqldump` 全量 + binlog 增量 |
| Redis | `redis_data/` | 启用 AOF；按需 RDB 快照 |
| 密钥 | `secrets/` | 离线归档至 Vault / KMS |

```powershell
# MySQL 全量备份示例
docker exec mysql sh -c 'mysqldump -uroot -prootpass superpowers' > backup\superpowers_%date%.sql
```

---

## 8. 升级流程

```powershell
# 1. 拉取新镜像
docker compose pull

# 2. 滚动重启（先无状态服务）
docker compose up -d --no-deps token-server
docker compose up -d --no-deps api-gateway

# 3. 再启有状态前置
docker compose up -d --no-deps auth-service user-service

# 4. 验证
docker compose ps
curl http://<host>:3000/health
```

不兼容的 schema 变更需先执行迁移脚本，再升级服务。

---

## 9. 回滚

```powershell
# 回滚到指定镜像 tag
docker compose pull token-server:<old-tag>
docker compose up -d --no-deps token-server
```

数据库回滚使用备份 SQL 恢复；务必先停写入服务（api-gateway / auth-service）。

---

## 10. 故障排查速查

| 现象 | 排查命令 |
|---|---|
| 容器起不来 | `docker compose logs <svc>` |
| 健康检查不过 | `docker inspect --format '{{.State.Health.Status}}' <svc>` |
| JWKS 拉取失败 | `curl -v http://<host>:4000/.well-known/jwks.json` |
| 网关 502 | `docker compose logs api-gateway` 查上游服务注册 |
| MySQL 连不上 | `docker exec mysql mysqladmin ping -h localhost` |
| 路由冲突 | `duplicate match pattern` → `coder-route-validate` 校验 |

---

## 11. 安全清单

- [ ] `secrets/private.pem` 已生成且权限收紧
- [ ] MySQL `root` 密码已替换为强口令（修改 `docker-compose.yml`）
- [ ] 3306 / 6379 / 8500 仅内网可达（防火墙 / 安全组）
- [ ] Consul UI 开启 ACL
- [ ] 镜像仓库已配置 HTTPS 与认证
- [ ] 启用日志收集（`docker compose logs` → ELK / Loki）

---

## 12. 关联文档

- 路由配置：[`config/api-gateway/routes/`](./config/api-gateway/routes/)
- 服务源码：[`../services/`](file:///d:/projects/coder-micro-services/services)
- 项目总览：[`../AGENTS.md`](file:///d:/projects/coder-micro-services/AGENTS.md)
