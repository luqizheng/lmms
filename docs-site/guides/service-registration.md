# 服务注册与服务发现

> 现状（2026-08 起）：业务容器通过 **Docker labels** 由单一 `gliderlabs/registrator` 项目级容器自动注册 / 注销到 Consul。**业务进程不直接调用 Consul HTTP API**，**无 envoy sidecar、无 Consul Connect / mTLS**。历史 in-process `libs/consul-registrar` 与 mesh sidecar 方案已下线（详见 [`docs/superpowers/specs/2026-07-07-deprecate-consul-connect-use-registrator-design.md`](../../superpowers/specs/2026-07-07-deprecate-consul-connect-use-registrator-design.md)）。

## 0. 一句话

> `registrator` 容器挂 `/var/run/docker.sock`，监听容器启停，**从容器 labels**（`SERVICE_NAME` / `SERVICE_ID` / `SERVICE_PORT` / `SERVICE_<PORT>_CHECK_HTTP` 等）调用 Consul `/v1/agent/service/register` 注册服务，stop / kill 时调 `/v1/agent/service/deregister` 注销。业务进程零 Consul 依赖。

## 1. 启动顺序

```bash
docker compose -f infrastructure/docker-compose-infra.yml \
               -f infrastructure/docker-compose-services.yml up -d
```

Compose 顺序：

1. `consul`（dev 模式 `agent -dev`，HTTP 8500 / gRPC 8502）
2. `registrator`（`depends_on: consul: healthy`，挂 `/var/run/docker.sock`）
3. 业务容器（启动时被打上 `SERVICE_*` labels；registrator 监听 docker events 自动注册）

业务容器 `depends_on: consul (healthy)` 只是为了拿到稳定解析，不阻塞注册逻辑（registrator 内部有 retry + `-resync=30s`）。

## 2. 注册路径（label → Consul catalog）

### 2.1 registrator 服务定义

`infrastructure/docker-compose-infra.yml:22-35`：

```yaml
  registrator:
    image: ${BASE_REGISTRY}gliderlabs/registrator:master
    depends_on:
      consul:
        condition: service_healthy
    volumes:
      - /var/run/docker.sock:/tmp/docker.sock
    environment:
      DOCKER_HOST: unix:///tmp/docker.sock
      REGISTRATOR_RESYNC: "30s"
    command: ["consul://consul:8500"]
    restart: unless-stopped
    networks:
      - superpowers
```

> `:master` 在跑通后应固化到 sha256 digest（见 spec §10 风险）。当前未固化。

### 2.2 业务容器 labels（anchor 模板）

`infrastructure/docker-compose-services.yml` 在每个业务 service 上挂以下一组 labels（`x-svc-registrator-labels-*` anchor），例如：

```yaml
  iam-service:
    ...
    labels:
      - "SERVICE_NAME=iam-service"
      - "SERVICE_ID=iam-service"
      - "SERVICE_PORT=8081"
      - "SERVICE_TAGS=backend,iam-service"
      - "SERVICE_8081_CHECK_HTTP=/health"
      - "SERVICE_8081_CHECK_INTERVAL=15s"
      - "SERVICE_8081_CHECK_TIMEOUT=3s"
      - "logging=loki"          # Promtail SD opt-in（ADR-007 D2）
      - "service=iam-service"
      - "env=staging"
```

registrator 解析后对每个 `SERVICE_NAME` 发一次 `PUT /v1/agent/service/register`：

- `Name` = `SERVICE_NAME`
- `ID` = `SERVICE_ID`
- `Address` = 容器在 `superpowers` 网络的 IP（自动取）
- `Port` = `SERVICE_PORT`（**容器内端口**，不是 host 端口）
- `Tags` = `SERVICE_TAGS`（逗号分隔）
- `Check.HTTP` = `http://<container_ip>:<SERVICE_PORT><SERVICE_<PORT>_CHECK_HTTP>`，interval / timeout 按 labels

### 2.3 端口约定

| 角色 | 端口来源 | 说明 |
| --- | --- | --- |
| registrator 看到的 `SERVICE_PORT` | **容器内端口** | 业务进程 `bind 0.0.0.0:<port>`；不能写 host 端口 |
| `ports:`（compose host 映射） | 由 host 决定 | 只为宿主机直连调试；网关走容器内端口 |
| `SERVICE_<PORT>_CHECK_HTTP` 后缀 | 与 `SERVICE_PORT` **必须数字一致** | 例如 `SERVICE_PORT=8081` ↔ `SERVICE_8081_CHECK_HTTP=/health` |

### 2.4 自动注销 / 重对账

- 容器 `stop` / `kill` / `die`：registrator 通过 Docker events 调 `PUT /v1/agent/service/deregister/<SERVICE_ID>`
- `-resync=30s`：每 30s 重新对账容器 ↔ Consul catalog，应对 `docker-compose restart` / `docker kill` 后的瞬态不一致
- 同名复用 `SERVICE_ID` → 注册幂等

### 2.5 不再使用的旧字段（确认已下线）

- ❌ `ConsulRegistrar::new(...).register_service(...)`（业务代码已全部移除）
- ❌ `SERVICE_<PORT>_CHECK_TCP` / `-tags` 全局 tag 模板 / mTLS / `connect-proxy`（与本架构正交）
- ❌ `scripts/consul-intentions.sh`（consul-init 容器已删除）
- ❌ `* -sidecar` 容器与 `network_mode: "service:X-sidecar"`（已删除）

## 3. 校验（label 是否被 registrator 识别）

```bash
# 列出 Consul 内的 service 名（业务进程无 HTTP 也包含 outbox publisher 不含 —— 见 §6）
docker exec infrastructure-consul-1 consul catalog services

# 看某 service 的注册体（含 Address / Port / Check）
docker exec infrastructure-consul-1 consul catalog service iam-service

# 看健康检查最近一次结果
docker exec infrastructure-consul-1 consul health service iam-service

# 直接打业务端口（容器内端口！）
docker exec infrastructure-consul-1 curl -sf http://iam-service:8081/health | head -3

# 重对账：重启 registrator 让它重读 labels（极少用；正常情况下 labels 改变它会自动感知）
docker compose -f infrastructure/docker-compose.yml restart registrator
```

## 4. 网络拓扑

```
                 host:3000
                     │
                     ▼
         ┌──────────────────────┐
         │  api-gateway (:3000) │
         └──────────┬───────────┘
                    │  HTTP（明文，Authorization 透传 JWT）
                    ▼
   ┌──────────────────────────────────────┐
   │  iam-service / token-service / ...   │
   │  bind 0.0.0.0:<container-port>       │
   │  Consul 内 Address = 容器 superpowers IP
   └──────────────────────────────────────┘
                    ▲
                    │  label / event 监听
         ┌──────────┴───────────┐
         │  registrator         │ ──► consul (:8500) PUT /v1/agent/service/...
         └──────────────────────┘
```

服务名 → IP 通过 `consul catalog` 或 `<svc>.service.consul` DNS 解析；业务进程无 Consul 客户端代码。

## 5. 故障排查

| 现象 | 排查 |
| --- | --- |
| 服务未出现在 Consul | ① `docker inspect <container>` 看 `Config.Labels` 是否有 `SERVICE_*`；② `docker logs infrastructure-registrator-1 --tail 50` 看是否有 `add` / `remove` 事件；③ 确认 `consul` 已 healthy（registrator depends_on）|
| 注册成功但 check 一直 critical | ① `docker exec <container> curl -sf http://127.0.0.1:<SERVICE_PORT><CHECK_PATH>`（**容器内端口**）；② confirm `SERVICE_<PORT>_CHECK_HTTP` 的端口数字 = `SERVICE_PORT`；③ `/health` handler 不要查 DB（启动期 DB 未就绪会变 critical）|
| 容器重启后 service_id 漂移 | 当前架构下 `SERVICE_ID` 是 labels 写死的字符串（与容器 ID 无关），重启不变；如果漂移，说明 labels 被覆盖或 anchor 没引用 |
| 探针拿到错的 advertise host | 当前架构**不走 UDP 探针**了，Address 直接来自 docker inspect 拿到的容器 IP。如果某 service IP 错，多半是网络配置问题（不走 superpowers 网络）|
| Consul 挂了业务是否受影响 | 已注册服务的 DNS / catalog 缓存由 docker DNS 兜底（`<svc>:8081` 仍可解析）；新实例注册失败但 `-resync=30s` 会自动重试；不影响已运行实例的 HTTP 流量 |

## 6. 已知限制 / 边界

- **无 mTLS、无服务身份认证**：服务间流量明文 HTTP。跨服务调用的身份仍走 `Authorization: Bearer <jwt>`（见 `.trae/rules/auth-rules.md`）。生产 TLS 终结放到 ingress / LB（不在本指南范围）。
- **只做客户端注册，没有 sidecar / 数据面代理**：TLS、熔断、重试等能力由业务侧库（`libs/common-http` 等）承担，不依赖 envoy。
- **`iam-outbox-publisher` / `notification-outbox-publisher` 无 HTTP listener，不打 `SERVICE_*` labels**——它们只读 DB / RabbitMQ，不在 Consul catalog 出现。
- **`config-service` / `file-service` 源码存在但未部署**，启用时按 §2.2 模板加 labels 即可，不需要 `ConsulRegistrar`。
- **registrator master tag** 当前未固化到 digest（spec §10 风险登记）：升级镜像前先在 staging 跑一周。
- 单测（testcontainers）不启动 registrator，只跑业务逻辑——业务代码本身已无 Consul 依赖。