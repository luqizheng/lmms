# 服务注册与运维（Consul 直连）

> 现状：业务进程通过 `libs/consul-registrar` 直接调用 Consul HTTP API 完成注册/注销，**不部署 envoy sidecar，不使用 Consul Connect / mTLS**。历史 sidecar / mTLS 方案已下线。

## 1. 启动顺序

```bash
docker compose -f infrastructure/docker-compose-infra.yml \
               -f infrastructure/docker-compose-services.yml up -d
```

Compose 中业务容器 `depends_on: consul (healthy)`。业务进程启动后自行 PUT `/v1/agent/service/register`。

## 2. 注册路径

`libs/consul-registrar::ConsulRegistrar`：

- `new(addr, service_name, service_id_file)` 从 `service_id_file` 读回上次的 service_id；无则生成 `{service_name}-{uuid}` 缓存在内存。
- `register_discovered(port, fallback)`：
  1. UDP 探针探本机对 consul 的出口 IP（`discover_advertise_host`），失败回落到 `fallback`（一般传服务名，靠 Docker DNS）。
  2. 首次调用时把 service_id 落盘（即便 Consul 暂不可达也先持久化，避免下次重启换 id）。
  3. PUT `/v1/agent/service/register`，注册 `HTTP /health` check：`interval=10s, timeout=5s, DeregisterCriticalServiceAfter=1m`。
- `deregister()`：进程收到 shutdown 信号时 PUT `/v1/agent/service/deregister/{id}`。

各服务在 `main.rs` 里的调用模板：

```rust
let mut registrar = ConsulRegistrar::new(&cfg.consul.addr, &cfg.consul.service_name, &cfg.consul.service_id_file);
registrar.register_discovered(cfg.server.port, &cfg.consul.service_name).await?;
// ... run server ...
let _ = registrar.deregister().await;
```

## 3. 配置项

| 变量 / 字段 | 说明 |
|-------------|------|
| `CONSUL_ADDR` / `consul.addr` | Consul agent 基址，如 `http://consul:8500` |
| `consul.service_name` | 注册到 Consul 的服务名（也是 DNS 名） |
| `consul.service_id_file` | service_id 持久化路径，必须落在持久卷上（`/tmp` 会随容器重建被清） |
| `server.port` / `SERVER_PORT` | 业务进程监听端口，写进注册体的 `Port` |
| `SERVER_HOST` | 一般 `0.0.0.0`；容器间直接走 Docker DNS，无 sidecar netns 约束 |

## 4. 网络拓扑

```
                host:3000
                    │
                    ▼
        ┌──────────────────────┐
        │  api-gateway (:3000) │
        └──────────┬───────────┘
                   │  HTTP（明文）
                   ▼
   ┌──────────────────────────────────────┐
   │  iam-service / token-service / ...   │
   │  监听 0.0.0.0:<port>                 │
   │  在 Consul 注册 name + host:port     │
   └──────────────────────────────────────┘
                   ▲
                   │ 健康检查
        ┌──────────┴───────────┐
        │  consul (:8500)      │
        └──────────────────────┘
```

Gateway 通过 Consul 或 DNS 解析下游服务名后直连业务端口（明文 HTTP）。

## 5. 故障排查

| 现象 | 排查 |
|------|------|
| 服务未出现在 Consul | 看业务进程日志里 `consul-registrar` 相关 `error!` / `warn!`；确认 `CONSUL_ADDR` 可达 |
| 注册成功但 check 一直 critical | `curl http://<svc>:<port>/health`；确认端口在容器网络可达、`/health` 200 |
| 重启后 service_id 变了 | `service_id_file` 落在了非持久路径（如 `/tmp`）；改到挂载卷 |
| 探针拿到错的 advertise host | 传给 `register_discovered` 的 `fallback` 用服务名，让 Docker DNS 兜底 |
| Consul 挂了业务是否受影响 | 已注册服务照常提供 HTTP；新实例注册失败但会重试；`deregister` 失败不影响关停 |

## 6. 已知限制 / 边界

- **无 mTLS、无服务身份认证**：服务间流量明文 HTTP。跨服务调用的身份仍走 `Authorization: Bearer <jwt>`（见 `.trae/rules/auth-rules.md`）。
- **只做客户端注册，没有 sidecar / 数据面代理**：TLS、熔断、重试等能力由业务侧库（`libs/digua_common-http` 等）承担，不依赖 envoy。
- `iam-outbox-publisher` / `notification-outbox-publisher` 无 HTTP listener，不注册。
- `config-service` / `file-service` 源码存在但未部署，启用时按上述模板加 registrar 调用即可。
- 单测（testcontainers）不注册 Consul，只跑业务逻辑。
