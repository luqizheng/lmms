# 51 — Consul Connect 服务网格验收

> Phase 5-A: mTLS + 意图 + mesh 可观测性

## 前置

```bash
cd infrastructure
docker-compose -f docker-compose-infra.yml -f docker-compose-services.yml -f docker-compose-observability.yml up -d
sleep 30
```

## 验收步骤

### 1. Sidecar 全部就绪

```bash
docker-compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml ps | grep sidecar
```
所有 `<svc>-sidecar` 容器 `running`。

### 2. consul-init 完成 CA + 意图初始化

```bash
docker logs infrastructure-consul-init-1
# 期望输出: consul-init complete
```

```bash
docker exec infrastructure-consul-1 consul intention list
```
9 条 allow + 1 条 `* → * deny`。

### 3. e2e 全量回归

```bash
node scripts/run-e2e-local.js
```
零失败。gateway → 下游通过 mesh mTLS。

### 4. 非 mTLS 直连被拒

```bash
docker run --rm --network superpowers alpine/curl \
  curl -s -o /dev/null -w "%{http_code}" http://iam-service-sidecar:9081/health
```
期望: 非 200 / 连接拒绝。

### 5. 意图 deny 生效

file-service 无 iam-service 的 upstream（本地无 9081 监听），无法发起调用。

### 6. Mesh metrics 可观测

- Consul UI (`http://localhost:8500`) → Connect → 查看 sidecar proxy 状态
- Prometheus (`http://localhost:9090`) → `envoy_cluster_upstream_cx_connect_fail` 应为 0

## 验收完成

全部步骤通过 → Phase 5-A 完成。
