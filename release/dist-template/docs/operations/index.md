# 运维与部署指南

本文档面向**已获取分发包（tar.gz）的运维人员**，覆盖从解压到日常运维的全流程。
如果你拿到的是未打包的源码目录，直接从「第二步：配置与启动」开始。

---

## 第一步：获取与解压

### 在线环境（可直接访问 registry）

```bash
# 从内部文件服务器或 CI 产物下载
# 假设你拿到了 coder-micro-services-v1.0.0-airgap.tar.gz

tar -xzf coder-micro-services-v1.0.0-airgap.tar.gz
cd coder-micro-services-v1.0.0
```

> 在线环境下，Docker 可直接从 `REGISTRY`（默认 `zhcoder-docker-registry.com:8000`）拉取镜像，
> 无需执行 `node bin/load-images.js`。

### 离线环境（无外网 / 私有化部署）

```bash
tar -xzf coder-micro-services-v1.0.0-airgap.tar.gz
cd coder-micro-services-v1.0.0

# 加载所有 Docker 镜像（约 10-15GB，耗时 5-10 分钟）
node bin/load-images.js

# 验证镜像已加载
docker images | grep micro-server
```

### 传输到目标服务器

```bash
# SCP
scp coder-micro-services-v1.0.0-airgap.tar.gz user@target-server:/opt/

# USB / 内网文件共享
cp coder-micro-services-v1.0.0-airgap.tar.gz /mnt/usb/
```

---

## 第二步：配置与启动

### 2.1 生成密钥与密码

```bash
make secrets
```

`make secrets` 会自动完成以下工作：

| 操作 | 说明 |
|------|------|
| 生成 RSA-2048 JWT 密钥对 | `secrets/jwt_private.pem` + `jwt_public.pem` |
| 设置私钥权限 | `chmod 600 secrets/jwt_private.pem` |
| 生成随机密码 | MySQL root、MySQL app、Redis、RabbitMQ |
| 生成 bootstrap token | 8 个核心服务各一个，每个 64 hex 字符 |
| 写入 `.env` | 从 `.env.example` 复制并替换所有 `__CHANGE_ME__` 占位符 |

> **生产环境必须手动复核 `.env` 中的密码**，确认所有敏感值已替换。

### 2.2 启动所有服务

```bash
make up
```

此命令会：

1. 构建前端镜像（首次 ~2 分钟，后续命中 Docker cache 秒级）
2. `docker compose up -d` 启动全部 18 个容器
3. 等待关键服务（MySQL / Consul / Redis / RabbitMQ / API 网关）healthy（最多 120s）

### 2.3 验证

```bash
make verify
```

预期输出所有服务 `✓` 通过：

```
=== verify: HTTP 端点检查 ===
  ✓ api-gateway
  ✓ config-service
  ✓ file-service
  ✓ token-service
  ✓ iam-service
  ✓ audit-service
  ✓ frontend
  ✓ prometheus
  ✓ grafana
结果: 9 通过, 0 失败
```

### 2.4 访问地址

| 服务 | 地址 | 默认账号 |
|------|------|---------|
| 前端 | `http://localhost:3000` | — |
| API 网关 | `http://localhost:8080` | — |
| Consul | `http://localhost:8500` | — |
| Prometheus | `http://localhost:9090` | — |
| Grafana | `http://localhost:3001` | admin / admin |
| Jaeger | `http://localhost:16686` | — |
| RabbitMQ 管理 | `http://localhost:15672` | guest / guest |

---

## 第三步：日常运维

### 启停命令速查

```bash
make up          # 启动所有服务
make down        # 停止所有容器（保留数据卷）
make status      # 列出容器状态（含健康检查）
make logs        # 查看所有服务实时日志
make logs svc=api-gateway   # 查看指定服务日志
```

### 查看容器状态

```bash
make status
```

正常输出应显示 18 个容器全部 `Up` 且关键服务 `healthy`：

| 容器名 | 期望状态 |
|--------|---------|
| `coder-micro-services-mysql` | Up (healthy) |
| `coder-micro-services-redis` | Up (healthy) |
| `coder-micro-services-consul` | Up (healthy) |
| `coder-micro-services-rabbitmq` | Up (healthy) |
| `coder-micro-services-api-gateway` | Up (healthy) |
| ... 其他服务 | Up (healthy / running) |

### 查看日志

```bash
# 实时跟踪
make logs

# 最近 500 行
docker compose -f infrastructure/docker-compose.yml logs --tail 500

# 特定服务
make logs svc=token-service
docker compose -f infrastructure/docker-compose.yml logs -f token-service
```

### 查看资源使用

```bash
docker stats --no-stream
```

---

## 第四步：配置调优

### 4.1 端口映射

编辑 `.env` 修改对外端口：

```env
API_GATEWAY_PORT=3000       # 前端访问端口
FRONTEND_PORT=8080          # 前端 nginx 端口
MYSQL_PORT_EXPORT=3306      # MySQL 对外端口
```

修改后重启：

```bash
make down && make up
```

### 4.2 限流规则

API 网关限流配置位于 `infrastructure/config/api-gateway/rate-limits.toml`：

```toml
[defaults]
requests_per_second = 100    # 全局默认限流
burst = 200                  # 突发容量

[routes."/v1/iam/auth/login"]
requests_per_second = 5      # 登录端点严格限制
burst = 10
```

修改后重启网关：

```bash
docker compose -f infrastructure/docker-compose.yml restart api-gateway
```

### 4.3 网关路由

路由规则位于 `infrastructure/config/api-gateway/routes/*.toml`。

新增路由需要同时：
1. 添加 `routes/<name>.router.toml` 规则文件
2. 重启网关使规则生效

### 4.4 数据卷

所有持久化数据存储在 Docker volumes 中：

| Volume | 内容 | 大小预估 |
|--------|------|---------|
| `mysql_data` | MySQL 数据 | 随业务增长 |
| `redis_data` | Redis 持久化 (AOF) | 数百 MB |
| `rabbitmq_data` | RabbitMQ 消息/定义 | 数百 MB |
| `file-storage` | 上传文件 | 随业务增长 |
| `grafana_data` | Grafana 看板 | 数十 MB |
| `prometheus_data` | Prometheus 时序 | 数 GB（建议设 retention） |

查看 volume 占用：

```bash
docker system df -v | grep coder
```

---

## 第五步：备份与恢复

详见 [`deployment/backup.md`](../deployment/backup.md)，以下为速查：

### 一键备份

```bash
# 导出 MySQL 全量
docker exec coder-micro-services-mysql mysqldump -uroot -p"${MYSQL_ROOT_PASSWORD}" \
  --all-databases --single-transaction > backup-$(date +%Y%m%d).sql

# Redis 备份
docker exec coder-micro-services-redis redis-cli -a "${REDIS_PASSWORD}" BGSAVE

# RabbitMQ 导出定义
docker exec coder-micro-services-rabbitmq rabbitmqctl export_definitions \
  /tmp/definitions.json
docker cp coder-micro-services-rabbitmq:/tmp/definitions.json backup-definitions-$(date +%Y%m%d).json
```

### 恢复

```bash
# MySQL 恢复
docker exec -i coder-micro-services-mysql mysql -uroot -p"${MYSQL_ROOT_PASSWORD}" < backup.sql

# RabbitMQ 恢复
docker cp backup-definitions.json coder-micro-services-rabbitmq:/tmp/
docker exec coder-micro-services-rabbitmq rabbitmqctl import_definitions /tmp/backup-definitions.json
```

---

## 第六步：升级

详见 [`deployment/upgrade.md`](../deployment/upgrade.md)，以下为速查：

### 在线升级

```bash
# 修改 .env 中的 IMAGE_TAG
sed -i 's/IMAGE_TAG=v1.0.0/IMAGE_TAG=v1.1.0/' .env
make upgrade TAG=v1.1.0
make verify
```

### 离线升级

```bash
# 1. 传输新镜像包到目标机器
# 2. 加载镜像
node bin/load-images.js

# 3. 改 tag 并重启
sed -i 's/IMAGE_TAG=v1.0.0/IMAGE_TAG=v1.1.0/' .env
make down && make up
```

### 回滚

```bash
sed -i 's/IMAGE_TAG=v1.1.0/IMAGE_TAG=v1.0.0/' .env
make down && make up
```

> 数据迁移仅在 MySQL 数据目录为空时自动执行。降级后若 schema 不兼容，可能需要 `make reset-data`（**会清除所有数据**）。

---

## 第七步：故障排查

详见 [`troubleshooting.md`](../troubleshooting.md)，以下为高频问题：

### 容器反复重启

```bash
# 查看具体错误
docker logs coder-micro-services-<服务名> --tail 50

# 常见原因：
# 1. 依赖未就绪 → make status 检查 MySQL/Redis/Consul/RabbitMQ 是否 healthy
# 2. 密码错误 → 检查 .env 中密码是否已替换
# 3. 端口冲突 → netstat -tlnp | grep -E "8080|3000|8500"
```

### 前端 502 / 无法访问

```bash
# 检查网关健康
curl http://localhost:8080/health

# 检查前端容器状态
docker logs coder-micro-services-frontend --tail 20

# 检查网关到后端服务的连通性
docker exec coder-micro-services-api-gateway curl -s http://token-service:8082/health
```

### MySQL 连接拒绝

```bash
# 检查 MySQL 是否 ready
docker exec coder-micro-services-mysql mysqladmin ping -uroot -p

# 查看 MySQL 日志
docker logs coder-micro-services-mysql --tail 30
```

### 数据重置

```bash
# ⚠️ 清除所有数据，不可逆
make reset-data
```

---

## 附录：完整容器清单

| 容器名 | 镜像 | 端口 |
|--------|------|------|
| `coder-micro-services-mysql` | mysql:8.0 | 3306 |
| `coder-micro-services-redis` | redis:7-alpine | 6379 |
| `coder-micro-services-consul` | hashicorp/consul:2.0.1 | 8500/8502 |
| `coder-micro-services-rabbitmq` | rabbitmq:3.13-management-alpine | 5672/15672 |
| `coder-micro-services-registrator` | gliderlabs/registrator:master | — |
| `coder-micro-services-api-gateway` | micro-server/api-gateway | 8080 |
| `coder-micro-services-config-service` | micro-server/config-service | 8081 |
| `coder-micro-services-file-service` | micro-server/file-service | 8082 |
| `coder-micro-services-token-service` | micro-server/token-service | 8083 |
| `coder-micro-services-iam-service` | micro-server/iam-service | 8084 |
| `coder-micro-services-notification-service` | micro-server/notification-service | 8085 |
| `coder-micro-services-audit-service` | micro-server/audit-service | 8086 |
| `coder-micro-services-prue-backend` | micro-server/prue-admin-backend | 8087 |
| `coder-micro-services-frontend` | micro-server/frontend | 3000 |
| `coder-micro-services-prometheus` | prom/prometheus:v2.51.0 | 9090 |
| `coder-micro-services-grafana` | grafana/grafana:10.4.0 | 3001 |
| `coder-micro-services-jaeger` | jaegertracing/all-in-one:1.55 | 16686 |
| `coder-micro-services-otel-collector` | otel/opentelemetry-collector-contrib:0.96.0 | 4317/4318 |

---

## 附录：系统要求

| 资源 | 最低 | 推荐 |
|------|------|------|
| CPU | 4 核 | 8 核 |
| 内存 | 8 GB | 16 GB |
| 磁盘 | 20 GB（不含镜像） | 50 GB+ |
| Docker | 24.0+ | 最新稳定版 |
| Docker Compose | 2.20+（`docker compose` v2） | 最新稳定版 |
| 操作系统 | Linux x86_64 | Ubuntu 22.04 / Debian 12 |

> 镜像包（`images/`）约 10-15GB，解压后另需同等空间。建议预留 50GB 以上总磁盘。
