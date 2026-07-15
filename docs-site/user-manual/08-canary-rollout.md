# 08. 灰度发布

> Consul 打 tag + Nginx `split_clients` 切 10% 流量给金丝雀。
> 出问题？1 秒回滚。

---

## 8.1 机制

- Consul 服务发现：实例带 `canary` tag
- Nginx `split_clients $request_id`：10% 流量切到金丝雀
- 强制金丝雀：`?canary=1` 或 `X-Canary: true` header
- 健康检查：Consul TTL 把不健康的金丝雀从 upstream 摘掉

```
                                ┌─── stable-1 (90%)
[Nginx] ── split_clients ──────┤
                                └─── canary-1  (10%)
                                       ↑
                                  Consul tag = canary
```

---

## 8.2 部署金丝雀实例

**方式 A：临时打 tag（快，适合已有实例测试）**

```bash
docker exec consul consul tags -add \
  -service audit-service \
  -tag canary \
  -id <instance-id>
```

**方式 B：独立部署一份（标准灰度）**

```bash
docker compose -f infrastructure/docker-compose-canary.yml up -d audit-service-canary
```

---

## 8.3 在 Jaeger 验证

```bash
# 打 100 次，约 10% 命中金丝雀
for i in {1..100}; do curl -s http://localhost:3000/v1/audit/health; done

# 强制命中金丝雀
curl -H "X-Canary: true" http://localhost:3000/v1/audit/health
# → Jaeger trace 里 instance=audit-service-canary
```

---

## 8.4 回滚（≈ 1 秒）

```bash
# 方式 A：删 tag
docker exec consul consul tags -delete \
  -service audit-service \
  -tag canary \
  -id <instance-id>

# 方式 B：下线金丝雀实例
docker compose -f infrastructure/docker-compose-canary.yml down

# 约 10 秒后，Nginx 上游全部回到 stable，流量 100% 稳定版
```

---

## 8.5 灰度发布流程（推荐）

```
1. 部署金丝雀实例 → 走 §8.2 方式 B
2. 在 Jaeger / Grafana 盯金丝雀的：
   - 错误率（应与稳定版同量级）
   - P99 延迟
   - 限流命中率（不应突增）
3. 手动请求（X-Canary: true）跑核心路径
4. 持续观察 15~30 分钟
5. 没问题 → 滚动升级稳定实例（金丝雀变成新稳定版）
6. 有问题 → §8.4 一秒回滚
```

---

## 8.6 进阶：流量比例动态调整

`split_clients` 默认 10%。要调比例：

1. 编辑 `infrastructure/config/nginx/conf.d/canary.conf`
2. 修改权重（如 `90%:10%` → `70%:30%`）
3. `nginx -s reload`（容器内）或 `docker exec nginx nginx -s reload`

> 完整 runbook 见 [`docs/guides/canary-rollout.md`](../guides/canary-rollout.md)。
