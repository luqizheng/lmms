---
name: "coder-docker-pull"
description: "Helps pull Docker images from alternative registries when default pull fails. Invoke when docker pull fails, network timeout, or user asks to pull images from mirror."
---

# coder-docker-pull

## 何时用

- Docker 镜像拉取失败（网络超时、连接拒绝、镜像不存在）
- 用户说「拉取镜像失败」「docker pull 失败」「换镜像源」
- 需要从备用镜像仓库拉取官方镜像或私有镜像

## 前置

- Docker Desktop 已启动
- 已知本系统需要的镜像清单

## 步骤

### 1. 识别失败镜像

从错误信息中提取失败的镜像名称，例如：
```
Error response from daemon: pull access denied for xxx
Error response from daemon: Get "https://registry-1.docker.io/v2/": net/http: TLS handshake timeout
```

### 2. 询问用户镜像服务地址

使用 `AskUserQuestion` 询问用户：
- 镜像服务地址（如 `buvzad01wui9xdo5lg.xuanyuan.run` 或 `zhcoder-docker-registry.com:8000/official`）
- 是否需要认证（用户名/密码）

### 3. 登录镜像仓库（如需要）

```bash
docker login <registry-address>
```

### 4. 拉取镜像

从备用地址拉取镜像：

```bash
# 官方镜像示例（如 redis:7-alpine）
docker pull <registry-address>/redis:7-alpine

# 私有镜像示例
docker pull <registry-address>/micro-server/token-service:latest
```

### 5. 重新打标签

将镜像标记为本系统需要的名称：

```bash
# 官方镜像
docker tag <registry-address>/redis:7-alpine redis:7-alpine

# 私有镜像
docker tag <registry-address>/micro-server/token-service:latest zhcoder-docker-registry.com:8000/micro-server/token-service:latest
```

### 6. 验证镜像

```bash
docker images | grep <image-name>
```

## 本系统镜像清单

### 官方镜像（Docker Hub）

| 镜像名称 | 用途 |
|---------|------|
| `consul:1.15` | 服务发现 |
| `hashicorp/consul:1.20` | 服务发现（新版） |
| `redis:7-alpine` | 缓存 |
| `mysql:8.0` | 数据库 |
| `nginx:alpine` | 反向代理 |

### 私有镜像（zhcoder-docker-registry.com:8000）

| 镜像名称 | 用途 |
|---------|------|
| `zhcoder-docker-registry.com:8000/micro-server/api-gateway:latest` | API 网关 |
| `zhcoder-docker-registry.com:8000/micro-server/token-service:latest` | Token 服务 |
| `zhcoder-docker-registry.com:8000/coder/user-service:latest` | IAM 服务 |
| `zhcoder-docker-registry.com:8000/micro-server/user-service:latest` | 通知服务 |

### 编译镜像（Rust 构建）

| 镜像名称 | 用途 |
|---------|------|
| `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl` | Rust 编译环境 |

## 常用场景

### 场景 1：拉取官方镜像失败

```bash
# 1. 从镜像服务拉取
docker pull buvzad01wui9xdo5lg.xuanyuan.run/library/redis:7-alpine

# 2. 重新打标签
docker tag buvzad01wui9xdo5lg.xuanyuan.run/library/redis:7-alpine redis:7-alpine

# 3. 清理临时镜像
docker rmi buvzad01wui9xdo5lg.xuanyuan.run/library/redis:7-alpine
```

### 场景 2：拉取私有镜像失败

```bash
# 1. 登录私有仓库
docker login zhcoder-docker-registry.com:8000

# 2. 拉取镜像
docker pull zhcoder-docker-registry.com:8000/micro-server/token-service:latest
```

### 场景 3：批量拉取所有镜像

```bash
# 官方镜像
docker pull consul:1.15
docker pull redis:7-alpine
docker pull mysql:8.0
docker pull nginx:alpine

# 私有镜像
docker pull zhcoder-docker-registry.com:8000/micro-server/api-gateway:latest
docker pull zhcoder-docker-registry.com:8000/micro-server/token-service:latest
docker pull zhcoder-docker-registry.com:8000/coder/user-service:latest
docker pull zhcoder-docker-registry.com:8000/micro-server/user-service:latest
```

### 场景 4：网络抖动时绕过镜像拉取

```bash
# 使用已缓存的本地镜像启动
docker compose up -d --pull never
```

## 故障排查

### 问题 1：TLS handshake timeout

**原因**：网络连接 Docker Hub 超时

**解决**：
1. 使用国内镜像源
2. 配置 Docker daemon 的 registry-mirrors

### 问题 2：pull access denied

**原因**：私有镜像需要认证或权限不足

**解决**：
```bash
# 登录私有仓库
docker login zhcoder-docker-registry.com:8000

# 输入用户名和密码
```

### 问题 3：镜像不存在

**原因**：镜像名称错误或镜像未推送

**解决**：
1. 检查镜像名称是否正确
2. 确认镜像已推送到仓库
3. 联系镜像管理员

### 问题 4：磁盘空间不足

**原因**：Docker 镜像占用过多空间

**解决**：
```bash
# 清理未使用的镜像
docker image prune -a

# 清理所有未使用资源
docker system prune -a
```

## 重要约束

**禁止事项**：
- ❌ 不要修改 `Dockerfile` 中的镜像名称
- ❌ 不要修改 `docker-compose.yml` 中的镜像名称
- ❌ 不要修改任何配置文件中的镜像引用

**允许操作**：
- ✅ 使用 `docker pull` 从备用地址拉取
- ✅ 使用 `docker tag` 重新标记镜像
- ✅ 使用 `docker login` 登录私有仓库
- ✅ 配置 Docker daemon 的镜像加速器

## 镜像加速器配置（可选）

编辑 Docker Desktop 设置或 `/etc/docker/daemon.json`：

```json
{
  "registry-mirrors": [
    "https://buvzad01wui9xdo5lg.xuanyuan.run",
    "https://zhcoder-docker-registry.com:8000/official"
  ]
}
```

重启 Docker 服务后生效。