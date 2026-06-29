# Phase 5-A — mTLS 服务间安全（Consul Connect Sidecar）

> 日期：2026-06-29
> 来源：TODO「Phase 5 — 服务间安全 & 韧性（P1）」中的「A = mTLS / 服务网格」条目。
> Phase 5 拆为两个独立子项目：B = 韧性三件套（已完成）、A = mTLS（本 spec）。
>
> Brainstorm 结论已 bake：Consul Dataplane Sidecar + Consul 内置 CA + `network_mode: service`。

## 1. 背景与问题

当前 7 个 Rust 业务服务（api-gateway / token-service / iam-service / notification-service / audit-service / config-service / file-service）之间通过 **应用层 service token** 认证：

- `libs/authz/src/require_service_token.rs` 在部分服务端点提供中间件保护。
- Service token 为 JWT，各服务持有公私钥，校验 `token_type == "service"` + audience。

**问题**：

1. **应用层 token 复杂**：每个服务需管理公钥、处理 JWT 验签、维护 audience 白名单。
2. **无传输加密**：内网通信为明文 HTTP，被嗅探即暴露。
3. **中间人攻击风险**： Consul 服务发现返回 IP 后直接连，无身份验证。

## 2. 目标 / 非目标

### 目标

- 引入 **Consul Connect** mTLS，所有服务间通信双向 TLS 加密 + 身份认证。
- 使用 **Consul Dataplane Sidecar**（轻量代理，~5-10MB 内存），与现有 `consul:1.15` 兼容。
- **删除** `require_service_token` 应用层中间件，mTLS 替代服务身份认证。
- docker-compose 开发环境全链路跑通；K8s 部署方案预留但不实施。

### 非目标（YAGNI）

- **不**引入服务网格级流量管理（金丝雀、重试、超时）—— 由 `ResilientClient` 负责。
- **不**做跨数据中心 mTLS 或 WAN gossip —— 单集群场景。
- **不**替换用户认证（JWT Bearer Token）—— 仅替换 service-to-service token。
- **不**做证书轮换 SOP 自动化 —— Consul CA 自动轮换，人工确认即可。

## 3. 决策摘要

| 决策点 | 选择 | 理由 |
|--------|------|------|
| Sidecar 选型 | Consul Dataplane | 内存低（~5-10MB），与现有 Consul 1.15 兼容，无需本地 consul agent |
| CA 选型 | Consul 内置 CA | 零运维，自动签发/轮换，适合当前规模 |
| 部署模式 | `network_mode: service:<svc>` | docker-compose 下 sidecar 与业务容器共享网络栈 |
| 上游暴露 | Sidecar ingress → localhost:业务端口 | 业务容器不改端口，sidecar 做 mTLS 终止 |
| 下游调用 | 业务代码 → `localhost:19001` | sidecar egress 做 mTLS 发起 |
| 应用层认证 | **删除** `require_service_token` | mTLS 已验证对端身份，JWT service token 冗余 |

## 4. 架构

```
┌─────────────────────────────────────────────────────────┐
│  api-gateway (port 3000 对外)                            │
│  ├─ axum router / auth middleware (user JWT)             │
│  ├─ ResilientClient ──► localhost:19001 (sidecar egress) │
│  └─ consul-dataplane (sidecar)                           │
│       ├─ ingress: 20000 (mTLS from upstream sidecars)    │
│       └─ egress:  19001 ──► mTLS ──► downstream sidecars │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│  iam-service (port 8080, 仅本机)                         │
│  ├─ axum handlers                                        │
│  └─ consul-dataplane (sidecar)                           │
│       ├─ ingress: 20000 ──► mTLS终止 ──► localhost:8080  │
│       └─ egress:  19001                                  │
└─────────────────────────────────────────────────────────┘
```

### 4.1 服务发现变更

Consul 注册时增加 `connect` 字段：

```json
{
  "ID": "iam-service",
  "Name": "iam-service",
  "Port": 8080,
  "connect": {
    "sidecar_service": {}
  }
}
```

Sidecar 自动注册为代理服务，Consul 为其分配 mTLS 证书。

### 4.2 网关转发变更

`api-gateway` 的 `forward_to_service` 不再直接连下游服务端口，而是连本机 sidecar egress：

```rust
// 变更前
let url = format!("http://{host}:{port}{path}", ...);

// 变更后：通过 sidecar egress 代理，sidecar 用 mTLS 连下游
let url = format!("http://localhost:19001{path}");
// 请求头加 `:authority` 或自定义头指示目标服务，sidecar 按 Consul SNI 路由
```

实际部署中，sidecar egress 会按 Consul 的 upstream 配置自动路由到目标服务的 sidecar ingress。

### 4.3 应用层认证淘汰

删除内容：

- `libs/authz/src/require_service_token.rs` 及单元测试
- `libs/authz/src/lib.rs` 中的 `pub mod require_service_token`
- 各服务端点中 `.layer(require_service_token(...))` 的引用

保留内容：

- `require_role`（用户权限校验，不受 mTLS 影响）
- 用户 JWT 认证（`Authorization: Bearer <user-jwt>`）

## 5. 部署（docker-compose）

### 5.1 基础设施层

`infrastructure/docker-compose-infra.yml` 增加 Consul Connect CA 初始化：

```yaml
  consul:
    image: consul:1.15
    # ... 现有配置
    environment:
      - CONSUL_CONNECT_ENABLED=true
    command: >
      agent -dev
      -config-dir=/consul/config
      -bind=0.0.0.0
      -client=0.0.0.0
      -hcl="connect { enabled = true }"
```

### 5.2 业务服务 + Sidecar

以 `iam-service` 为例：

```yaml
  iam-service:
    build: ./services/iam-service
    network_mode: service:iam-sidecar
    environment:
      - SERVER_ADDR=0.0.0.0:8080
      - CONSUL_ADDR=http://consul:8500
    depends_on:
      iam-sidecar:
        condition: service_healthy

  iam-sidecar:
    image: hashicorp/consul-dataplane:1.2
    environment:
      - DP_CONSUL_SERVER_ADDRESSES=consul:8500
      - DP_SERVICE_ID=iam-service
      - DP_SERVICE_UPSTREAM=localhost:8080
      - DP_PROXY_BIND_ADDR=0.0.0.0
      - DP_ENVOY_READY_BIND_ADDR=0.0.0.0
    ports:
      - "8080:20000"   # 外部仍走 8080，sidecar ingress 终止 mTLS
    networks:
      - superpowers
    depends_on:
      consul:
        condition: service_healthy
```

### 5.3 api-gateway 特殊处理

Gateway 对外暴露 3000，对内通过 sidecar egress 连下游：

```yaml
  api-gateway:
    build: ./services/api-gateway
    network_mode: service:gateway-sidecar
    environment:
      - SERVER_ADDR=0.0.0.0:3000
      - SIDECAR_EGRESS_URL=http://localhost:19001
    ports:
      - "3000:3000"    # 由 sidecar 映射或 gateway 直接暴露
```

Gateway sidecar 需配置所有下游 upstream：

```yaml
  gateway-sidecar:
    image: hashicorp/consul-dataplane:1.2
    environment:
      - DP_CONSUL_SERVER_ADDRESSES=consul:8500
      - DP_SERVICE_ID=api-gateway
      - DP_SERVICE_UPSTREAM=localhost:3000
      - DP_ENVOY_READY_BIND_ADDR=0.0.0.0
    ports:
      - "3000:3000"    # Gateway 对外端口直接暴露（或走 sidecar ingress）
      - "19001:19001"  # Egress 端口（gateway 代码连接用）
    networks:
      - superpowers
```

## 6. 证书生命周期

| 阶段 | 行为 | 负责人 |
|------|------|--------|
| 签发 | Consul CA 自动为每个 sidecar 签发 SPIFFE 证书 | Consul server |
| 轮换 | 默认 72h 有效期，自动在到期前轮换 | Consul dataplane |
| 吊销 | 服务注销时证书自动失效 | Consul server |
| 备份 | CA 根证书备份到 `docker/secrets/` | 运维 |

## 7. 风险与回滚

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| Consul CA 初始化失败 | 所有 sidecar 无法启动 | docker-compose healthcheck + `depends_on` 顺序控制 |
| Sidecar 内存泄漏 | 单个节点 OOM | 限制 sidecar 容器内存 64MB，超限时自动重启 |
| mTLS 握手失败 | 服务间调用 503 | sidecar 日志暴露 Envoy admin `/clusters` 供排查 |
| 回滚需求 | 需切回 service token | **保留** `require_service_token` 代码分支 2 周，通过 feature flag `MTLS_ENABLED` 切换 |

## 8. 验证方式

1. **握手验证**：`openssl s_client -connect iam-sidecar:20000 -showcerts` 能看到 Consul CA 签发的证书链。
2. **流量验证**：gateway → iam 请求正常返回 200，且 sidecar access log 显示 TLS 版本。
3. **负向验证**：停掉 iam-sidecar，gateway 请求失败（而非 fallback 到明文）。
4. **回归测试**：e2e 登录/权限/公告流程全量跑通。

## 9. 决策记录

**9.1 为什么不用 Linkerd2？**

Linkerd2 标准 sidecar 内存 ~50-100MB，超出本项目 "low-memory" 定位。Consul Dataplane ~5-10MB，更适合当前资源约束。

**9.2 为什么删除 `require_service_token` 而不是共存？**

mTLS 已提供双向身份认证 + 传输加密，应用层 service token 成为纯冗余。共存增加维护成本且无安全增益。若未来需额外鉴权（如细粒度服务权限），可在 mTLS 之上叠加 SPIFFE ID 校验，而非回退到 JWT。

**9.3 为什么用 `network_mode: service` 而非独立端口映射？**

docker-compose 下 sidecar 与业务容器需共享 localhost，才能让业务代码 `localhost:19001` 连到 sidecar egress。K8s 环境下改为 Pod 内 `127.0.0.1` 共享网络，概念一致。
