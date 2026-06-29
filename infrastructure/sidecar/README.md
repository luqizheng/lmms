# Consul Dataplane Sidecar 模板

> Phase 5-A mTLS 服务网格规约。所有业务服务伴生容器统一模板。

## Ingress 端口（业务容器对外入口）

每个 sidecar 暴露固定 `20000`，sidecar 自动把解密后的明文转给 `localhost:<业务端口>`。
业务容器本身仍监听 `0.0.0.0:8080`（gateway 是 3000），但**不**通过 docker port 映射对外，
由 sidecar 的 `ports:` 段做端口暴露。

| Service             | 业务端口 | sidecar ports                | 外部访问端口（host） |
|---------------------|----------|------------------------------|----------------------|
| iam-service         | 8080     | "6002:20000"                 | 6002                 |
| token-service       | 8080     | "6001:20000"                 | 6001                 |
| notification-service| 8080     | "6003:20000"                 | 6003                 |
| audit-service       | 8080     | "6005:20000"                 | 6005                 |
| prue_backend        | 8080     | "6004:20000"                 | 6004                 |
| api-gateway         | 3000     | "3000:3000" (业务进程明文)   | 3000                 |

> **gateway 例外**：api-gateway 是系统**边界**，对外接受用户 HTTP/JWT，无需对外 mTLS。所以
> gateway-sidecar **不开 ingress listener**（不要 :20000），只做 egress（5 个 upstream listener）。
> Host:3000 直接映射到业务进程的 3000，不经 envoy。

## Egress 端口（上游侧 sidecar）

仅 `api-gateway` 需要 egress，按下表声明 `proxy.upstreams[].local_bind_port`：

| 上游 destination_name | local_bind_port |
|-----------------------|-----------------|
| iam-service           | 18001           |
| token-service         | 18002           |
| notification-service  | 18003           |
| audit-service         | 18004           |
| prue_backend          | 18005           |

## Sidecar Compose 模板

声明在 `infrastructure/docker-compose-services.yml` 顶部的 anchors：

```yaml
x-sidecar-defaults: &sidecar-defaults
  image: hashicorp/consul-dataplane:1.2
  mem_limit: 64m
  depends_on:
    consul:
      condition: service_healthy
  networks:
    - superpowers

x-sidecar-env: &sidecar-env
  DP_CONSUL_ADDRESSES: consul
  DP_CONSUL_GRPC_PORT: "8502"
  DP_ENVOY_READY_BIND_ADDR: 0.0.0.0
  DP_ENVOY_READY_BIND_PORT: "20100"
  DP_ENVOY_CONCURRENCY: "1"
```

每个 sidecar 服务：

```yaml
  <svc>-sidecar:
    <<: *sidecar-defaults
    environment:
      <<: *sidecar-env
      DP_PROXY_SERVICE_ID: <service-name>-sidecar-proxy
    ports: ["<外部端口>:20000"]    # gateway 例外: "3000:3000"
```

业务容器：

```yaml
  <svc>:
    network_mode: service:<svc>-sidecar
    # 删除自身 ports:，由 sidecar 暴露
    depends_on:
      <svc>-sidecar:
        condition: service_started
```

## 注意事项

- `network_mode: service:X` 后业务容器**不能**再写 `networks:` 或 `ports:`。
- Consul gRPC 端口 8502 在 dev mode 默认启用；如自建集群需 `-grpc-port 8502` 显式开。
- Sidecar 启动需要 Consul 已注册对应 service + sidecar_service 定义，否则 envoy 无法配置 listener。
- **DNS 行为**：业务容器使用 `network_mode: service:<svc>-sidecar` 后**没有自己的网络栈**，docker 内部 DNS 对服务名 `<svc>` 的解析会返回 **sidecar 容器的 IP**（因为只有 sidecar 容器加入了 `superpowers` network）。这是正常工作模式：其他容器（如 `frontend`）通过 `http://api-gateway:3000` 仍能连到 gateway 业务进程，路径是 `frontend → gateway-sidecar netns → :3000 (gateway process)`。任何排障时如发现"DNS 解析到 sidecar IP" 属于预期行为，**不要**改动。
