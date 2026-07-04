# 51 — Service Mesh (Bridge + Direct) 验收

> Phase 5-A: 用 bridge 网络直连代替 Consul Connect sidecar（envoy 二进制在受限网络不可用）

## 前置

```bash
cd infrastructure
# 清理旧网络
docker network prune -f
# 启动
docker-compose -f docker-compose-infra.yml -f docker-compose-services.yml up -d
sleep 30
```

## 验收步骤

### 1. 所有服务运行

```bash
docker ps --format "{{.Names}}\t{{.Status}}" | grep -E "service|api"
```
所有服务 `Up`（iam-service, token-service, notification-service, audit-service, prue_backend, api-gateway）。

### 2. Consul 注册

```bash
docker exec infrastructure-consul-1 consul catalog services
```
包含: api-gateway, iam-service, token-service, notification-service, audit-service, prue_backend。

### 3. Docker DNS 连通性

```bash
# api-gateway → token-service JWKS
docker exec infrastructure-api-gateway-1 wget -qO- http://token-service:9082/.well-known/jwks.json | head -c 100
# 期望: 返回 JWKS JSON

# api-gateway → iam-service health
docker exec infrastructure-api-gateway-1 wget -qO- http://iam-service:9081/health
# 期望: OK
```

### 4. 网关路由

```bash
curl -s http://localhost:3000/health
# 期望: {"status":"ok"}

curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/api/v1/iam/permissions
# 期望: 401 (需要认证，不是 mesh 问题)
```

### 5. e2e 全量回归

```bash
node scripts/run-e2e-local.js
```
零失败。gateway → 下游通过 Docker DNS 直接通信。

## 验收完成

全部步骤通过 → 服务网格（bridge 模式）工作正常。
