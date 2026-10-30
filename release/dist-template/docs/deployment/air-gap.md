# 离线部署指南

本文档适用于**无法访问外网**的私有化部署环境。

## 前置条件

- 目标机器已安装 Docker 24.0+ 和 Docker Compose 2.20+
- 有网络环境机器用于打包镜像（通常在 CI/CD 服务器）
- 准备一个足够大的存储介质（建议 30GB+）用于传输镜像包

## 完整部署流程

### 第一步：在有网环境打包

在 CI 服务器或开发机器上执行：

```bash
# 克隆代码并 checkout 到目标版本
git clone <repo-url>
git checkout v1.0.0

# 执行打包脚本（开发仓库 根目录；镜像 tag 用 no-v: git tag `v1.0.0` → 镜像 tag `1.0.0`）
node release/dist-template/scripts/release.js 1.0.0   # 早期 shell 版 release.sh 已迁 Node 删除
```

输出文件：
- `release/coder-micro-services-1.0.0-airgap.tar.gz` — 完整离线包（含镜像 tar）
- `release/coder-micro-services-1.0.0/` — 解压后的目录

### 第二步：传输到目标机器

```bash
# 方式 1：SCP
scp coder-micro-services-1.0.0-airgap.tar.gz user@target:/opt/

# 方式 2：USB 拷贝
# 把 tar.gz 拷贝到 USB 盘，插到目标机器
```

### 第三步：解压并加载镜像

```bash
# SSH 到目标机器
ssh user@target-server

# 解压
cd /opt
tar -xzf coder-micro-services-1.0.0-airgap.tar.gz
cd coder-micro-services-1.0.0

# 加载 Docker 镜像（约 10-15GB，需 5-10 分钟）
node bin/load-images.js
```

### 第四步：初始化配置

```bash
# 复制并初始化 .env
cp .env.example .env
make secrets
```

> `make secrets` 会自动生成所有密钥和密码。**生产环境务必修改 `secrets/jwt_private.pem` 的权限**：
> ```bash
> chmod 600 secrets/jwt_private.pem
> ```

### 第五步：启动服务

```bash
# 启动所有服务——release.js 已把 compose（service + 主入口）中的 build: 转成 image: 引用，
# 且离线包已含全部镜像，无需现场构建：
node bin/start.js
```

### 第六步：验证

```bash
node bin/verify.js
```

预期输出：所有服务 `✓` 通过。

## 目录结构

```
coder-micro-services-1.0.0/
├── docker-compose.yml      # 主入口
├── .env                   # 环境变量（make secrets 生成）
├── compose/               # compose 模块
├── config/                # 配置文件
├── init-mysql/            # MySQL 初始化 SQL
├── secrets/               # 密钥（不提交 git）
├── bin/                   # 运维脚本
└── docs/                  # 本文档
```

## 网络要求

如果内部网络可以访问私有 registry（`zhcoder-docker-registry.com:8000`），可以跳过镜像加载步骤，直接 `make up`，Docker 会自动拉取镜像。

## 数据目录

所有数据默认存在 Docker volumes 中：

| Volume | 内容 |
|--------|------|
| `mysql_data` | MySQL 数据文件 |
| `redis_data` | Redis 持久化数据 |
| `rabbitmq_data` | RabbitMQ 数据 |
| `file-storage` | 上传文件存储 |
| `grafana_data` | Grafana 看板配置 |
| `prometheus_data` | Prometheus 时序数据 |

如需迁移到新机器，参考 [备份与恢复](./backup.md)。
