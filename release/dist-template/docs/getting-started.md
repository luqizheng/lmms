# 快速开始

## 环境要求

| 依赖 | 版本 | 说明 |
|------|------|------|
| Docker | 24.0+ | 容器运行时 |
| Docker Compose | 2.20+ | 编排工具（`docker compose` v2）|
| 磁盘 | 20GB+ | 镜像 + 数据卷 |
| 内存 | 8GB+ | 推荐 16GB |

## 在线部署（可访问 registry）

```bash
# 1. 解压发布包
tar -xzf coder-micro-services-v1.0.0-airgap.tar.gz
cd coder-micro-services-v1.0.0

# 2. 初始化 .env（自动生成密钥）
make secrets

# 3. 启动所有容器
make up

# 4. 验证
make verify
```

## 离线部署（air-gap）

```bash
# 1. 加载 Docker 镜像
bash bin/load-images.sh

# 2. 初始化 .env
make secrets

# 3. 启动
make up

# 4. 验证
make verify
```

## 访问地址

| 服务 | 地址 | 账号 |
|------|------|------|
| 前端 | http://localhost:8080 | — |
| API 网关 | http://localhost:3000 | — |
| Consul | http://localhost:8500 | — |
| Prometheus | http://localhost:9090 | — |
| Grafana | http://localhost:3001 | admin / admin |
| Jaeger | http://localhost:16686 | — |
| RabbitMQ | http://localhost:15672 | guest / guest |

## 常用命令

```bash
make status        # 查看容器状态
make logs [svc]    # 查看日志（可选指定服务）
make verify        # 端到端冒烟测试
make seed-demo     # 写入 demo 数据
make reset-data    # 清除所有数据并重置
make down          # 停止容器（保留数据卷）
make upgrade TAG=v1.x  # 升级到指定版本
```
