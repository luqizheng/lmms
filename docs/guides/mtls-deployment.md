# mTLS 服务网格部署与运维

> Phase 5-A 引入。基于 Consul Connect + consul-dataplane 1.2。

## 1. 启动顺序

```bash
docker compose -f infrastructure/docker-compose-infra.yml \
               -f infrastructure/docker-compose-services.yml up -d
```

Compose 已声明 `depends_on`：consul 优先，sidecar 等 consul healthy，业务容器等 sidecar started。

## 2. CA 根证书

```bash
# 备份根证书 PEM
curl -fsS http://localhost:8500/v1/connect/ca/roots | jq -r '.Roots[0].RootCertPEM' > ./docker-example/secrets/consul-ca-root.pem
```

dev 模式下 Consul 重启会重新生成 CA，所有 sidecar 自动重新签发；生产环境需挂持久化 data dir。

## 3. 证书轮换

默认叶证书 72h 有效期，到期前 60min 由 consul-dataplane 自动向 Consul 申请新证书并热替换。
人工触发轮换：

```bash
docker compose restart <svc>-sidecar
```

## 4. 端口规约

见 `infrastructure/sidecar/README.md`。

- 业务容器**只**与本机 sidecar 同 netns，**不**直接监听 host 端口。
- Gateway 的 egress 端口 `18001..18005` 只在 sidecar 容器内回环监听，gateway 业务进程访问 `127.0.0.1:1800X`。
- Gateway 边界例外：gateway-sidecar **不开 ingress mTLS**（业务进程直接监听 :3000，由 sidecar 容器暴露 `"3000:3000"` 给 host）。

## 4.1 网络拓扑（mTLS 后）

```
                            host:3000
                                │
                                ▼
   ┌────────────────────────────────────────────┐
   │  gateway-sidecar (持有 netns)             │
   │   ├─ :3000 ── (本地端口)── api-gateway 进程│  ◄── 外部明文（user JWT）
   │   ├─ :18001 ── envoy egress ──┐           │
   │   ├─ :18002 ── envoy egress ──┤           │
   │   ├─ :18003 ── envoy egress ──┤  mTLS out │
   │   ├─ :18004 ── envoy egress ──┤           │
   │   └─ :18005 ── envoy egress ──┘           │
   └──────────────────┬─────────────────────────┘
                      │ mTLS (Consul CA-signed)
                      ▼
   ┌──────────────────────────────────────┐
   │  iam-sidecar (持有 netns)            │
   │   ├─ :20000 ── envoy ingress ◄── mTLS terminate
   │   │                │
   │   │                ▼
   │   └─ :8080 ── iam-service 进程       │
   └──────────────────────────────────────┘
                      ▲
                      │ (重复 6 个下游 sidecar)
   token-sidecar / notification-sidecar / audit-sidecar / prue-sidecar / ...

   docker DNS 行为：业务服务名（如 "api-gateway"）解析到 sidecar 容器 IP，
                    因为业务容器走 network_mode: service 没有独立网络栈。
```

## 5. 故障排查

| 现象 | 排查命令 |
|------|----------|
| Sidecar 启动失败 | `docker compose logs <svc>-sidecar` 看 envoy bootstrap 错误 |
| envoy 无 cluster | `curl http://localhost:20100/clusters`（在容器内 exec） |
| mTLS 握手失败 | sidecar 日志搜 `TLS error`；确认 destination service name 在 Consul 已注册并健康 |
| gateway 502 | gateway 日志搜 `connect error`；确认对应 egress_map 端口存在 |
| Consul Connect 未启用 | `curl http://localhost:8500/v1/connect/ca/roots`，空数组即未启用 |
| **同网络容器绕过 sidecar 直连业务端口** | 业务进程绑了 `0.0.0.0` 而非 `127.0.0.1` —— 检查每个业务服务的 `SERVER_HOST` env / `http_addr` 配置，必须监听 sidecar netns 内回环（关键安全约束 4A）|

## 6. 回滚

回滚锚点：tag `pre-mtls` 打在 Task 12 commit（sidecar 全部部署完，`require_service_token` 尚未删除）。

| 场景 | 操作 |
|------|------|
| 只想保留 mTLS、恢复 service token 中间件 | `git revert <Task 13 commit hash>`（重新引入 `require_service_token`） |
| 整体退回 mTLS 之前 | `git reset --hard pre-mtls` 然后 `git revert HEAD..pre-mtls` 倒着撤所有 Task 1-12 改动；或直接基于 `pre-mtls` 之前的 commit 重建分支 |
| 临时绕过 mTLS（不改代码） | 在 `docker-compose-services.yml` 中把单个业务服务的 `network_mode: service:<svc>-sidecar` 改回 `networks: [superpowers]` + 恢复原 `ports:` 段，停掉对应 sidecar 容器 —— 流量降级为明文，但 gateway egress_map 仍指本机 sidecar 端口（连接失败 → 熔断 → 502），所以**必须**同时调整 gateway egress_map 临时指向 docker 服务名 |

## 7. 关键安全约束（必读）

**业务进程必须监听 sidecar netns 内回环地址（`127.0.0.1`），不能监听 `0.0.0.0`。**

否则同 docker 网络的其他容器（如 frontend、其他业务服务）可直连 `<svc>-sidecar:8080` **绕过 sidecar mTLS**，整个部署失去身份认证保护。

| 服务 | 配置入口 | 正确值 |
|------|----------|--------|
| iam-service | env `SERVER_HOST` | `127.0.0.1` |
| token-service | env `SERVER_HOST` | `127.0.0.1` |
| audit-service | env `SERVER_HOST` | `127.0.0.1` |
| prue_backend | env `SERVER_HOST` | `127.0.0.1` |
| notification-service | yaml `server.http_addr` | `127.0.0.1:8080` |
| api-gateway | env `GW_SERVER__HOST` | `0.0.0.0` （边界例外）|

Task 15 Step 2 的 3 条断言会校验此约束；上线前**必须**确认 3 条都通过。

## 8. 已知限制

- `iam-outbox-publisher` / `notification-outbox-publisher` 不接 sidecar（无 HTTP listener）。
- `config-service` / `file-service` 源码存在但未部署，启用时需按 `infrastructure/sidecar/README.md` 模板补端口规约。
- 测试容器（testcontainers）跑单测时不经 sidecar，仍走明文——单测只测业务逻辑，不验 mTLS。
