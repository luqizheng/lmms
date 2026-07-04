# 如何新增一个服务到 Consul Connect 服务网格

> 适用于需要入站 mTLS（被调用）或有出站调用（调用其他服务）的新服务。

## 前置知识

- **内端口**（180xx）：服务本身绑定 `127.0.0.1:180xx`，只在本 network namespace 内可达
- **外端口**（908x）：consul connect proxy 绑定 `0.0.0.0:908x`，docker-compose 映射到 host，这是被调用的入口
- 两者通过 `network_mode: service:X-sidecar` 共享网络栈，**端口不同，无冲突**

## 步骤

### 1. 给新服务分配端口号

选一个未被占用的 `180xx`（内）和 `908x`（外）：

| 已占用 | 内端口 | 外端口 |
|--------|--------|--------|
| api-gateway | 18000 | 3000 |
| iam-service | 18081 | 9081 |
| token-service | 18082 | 9082 |
| notification-service | 18083 | 9083 |
| audit-service | 18084 | 9084 |
| prue_backend | 18087 | 9087 |

例如你的新服务叫 `foo-service`，选 `18085 / 9085`。

### 2. 在 docker-compose-services.yml 加 sidecar + 主服务

```yaml
  # ─────────── foo-service + sidecar ───────────
  foo-service-sidecar:
    image: consul:1.15
    ports:
      - "9085:9085"
    environment:
      CONSUL_HTTP_ADDR: "http://consul:8500"
    command: >
      sh -c "until consul connect proxy -register -service foo-service
        -service-addr 127.0.0.1:18085 -listen ':9085'; do sleep 2; done"
    depends_on:
      consul:
        condition: service_healthy
    restart: unless-stopped
    networks:
      - superpowers

  foo-service:
    build:
      context: ..
      dockerfile: services/foo-service/Dockerfile
    network_mode: "service:foo-service-sidecar"
    environment:
      SERVER_HOST: "127.0.0.1"
      SERVER_PORT: "18085"
      # ... 其他环境变量
    depends_on:
      foo-service-sidecar:
        condition: service_started
      consul:
        condition: service_healthy
      # ... 依赖的服务（mysql, redis 等）
    # No networks: key here — incompatible with network_mode
```

**关键点**：
- sidecar 用 `networks:`，主服务用 `network_mode:`，两者**不能同时出现**
- sidecar 的 `-register` flag 让它自动注册到 Consul
- 主服务 `SERVER_HOST=127.0.0.1` 确保只监听内端口

### 3. 如果新服务需要调用其他服务

在 sidecar 命令中加 `-upstream` flag：

```yaml
    command: >
      sh -c "until consul connect proxy -register -service foo-service
        -service-addr 127.0.0.1:18085 -listen ':9085'
        -upstream iam-service:9081 -upstream token-service:9082; do sleep 2; done"
```

主服务配置中，用 `127.0.0.1:<外端口>` 访问上游（通过自己的 proxy）：

```
# foo-service 的 config.yaml 或 env var
iam_base_url: "http://127.0.0.1:9081"
```

### 4. 如果其他服务需要调用新服务

在**调用方**的 sidecar 命令中加 `-upstream foo-service:9085`：

```yaml
  api-gateway-sidecar:
    command: >
      sh -c "until consul connect proxy -register -service api-gateway
        ...
        -upstream foo-service:9085 ... ; do sleep 2; done"
```

更新调用方的 **egress_map**（如 api-gateway.toml）：

```toml
"foo-service" = { host = "127.0.0.1", port = 9085 }
```

### 5. 更新意图（intentions）

在 `scripts/consul-intentions.sh` 中添加 allow 规则：

```bash
# 允许 api-gateway 访问 foo-service
consul intention create api-gateway foo-service

# 如果 foo-service 需要调用 iam-service
consul intention create foo-service iam-service
```

注意：`* → * deny` 需要是最后一条。

### 6. 服务代码改动

#### 6.1 main.rs — ConsulRegistrar 注册

如果服务使用 `ConsulRegistrar`，加 upstreams 和 sidecar_config：

```rust
let mut consul_registrar = ConsulRegistrar::new(
    &consul_addr,
    "foo-service",
    "/tmp/foo-service-id",
)
.with_upstreams(vec![
    Upstream::new("iam-service", 9081),
    Upstream::new("token-service", 9082),
])
.with_sidecar_config("allow_untlsed_inbound", serde_json::json!(true));

consul_registrar
    .register_discovered(config.server.port, "foo-service")
    .await
    .err(); // best-effort: proxy 也会注册，不阻塞启动
```

> **ifupstreams 为空：不调用** 任何服务 → 可省略 `.with_upstreams()` 和 `.with_sidecar_config()`

#### 6.2 config.rs — 端口环境变量覆盖

确保 `SERVER_PORT` 环境变量可以覆盖 yaml 硬编码端口。参考 `services/iam-service/src/config.rs`：

```rust
fn default_port() -> u16 {
    std::env::var("SERVER_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(8080)
}
```

#### 6.3 config.yaml — 内端口

```yaml
server:
  host: "0.0.0.0"
  port: 18085    # 内端口
```

## 验证

```bash
# 1. 确认 proxy 在运行
docker ps --format "{{.Names}}" | grep foo-service-sidecar

# 2. 确认服务在 Consul 中注册
docker exec infrastructure-consul-1 consul catalog services | grep foo-service

# 3. 确认 mTLS 生效（直连应被拒）
curl -sv --max-time 3 http://localhost:9085/health 2>&1 | grep "Empty reply"

# 4. 如果 api-gateway 有 upstream，通过 egress_map 测试
docker exec infrastructure-api-gateway-1 wget -qO- -T 5 http://127.0.0.1:9085/health
```

## 常见问题

### Q: 为什么主服务用 `network_mode` 而不是 `networks:`？
`network_mode: service:X-sidecar` 共享 sidecar 的网络栈，使主服务能通过 `127.0.0.1` 访问 sidecar 的 upstream listener。如果用 `networks:`（bridge 模式），`127.0.0.1` 指向自己，无法访问 sidecar。

### Q: 为什么有两个端口（180xx 和 908x）？
因为同一 network namespace 内不能有两个进程监听同一端口。proxy 占用外端口（0.0.0.0:908x），主服务占用内端口（127.0.0.1:180xx），不冲突。

### Q: 健康检查为什么是 critical？
`ConsulRegistrar` 的 `discover_advertise_host` 在 UDP probe 失败时 fallback 到 hostname（如 `"foo-service"`），Consul DNS 无法解析。临时修复：服务启动后手动更新注册地址为 `"foo-service-sidecar"`。永久修复需 rebuild `libs/consul-registrar`。

### Q: 新服务不需要 mesh 可以吗？
可以。config-service 和 file-service 就是例子——直接 bridge 网络，无 sidecar。
