# 灰度发布运维手册（Canary Rollout）

> 任意服务新版本通过灰度发布逐步切流（10% → 50% → 100%）的分步指南。

## 适用场景

- 生产环境希望流量逐步切换
- 新版本可能有风险，需要用真实流量灰度验证
- 需要具备秒级回滚能力

## 架构

- **Nginx** 通过 Consul 服务发现把流量打到全部 8 个服务
- **consul-template** 每 2–10 秒轮询 Consul，重新渲染 nginx 配置
- 在 Consul 中带 `canary` 标签的服务实例初始拿到 10% 流量
- 请求带 `?canary=1` query 参数或 `X-Canary: true` header 时强制路由到 canary，便于灰度测试

## Step 1：部署 canary 实例

使用不同的服务名后缀，或者给已有实例打 canary 标签。

```bash
# 方式 A：通过 compose override 启一个独立的 canary 栈（推荐）
docker compose -f infrastructure/docker-compose-canary.yml up -d audit-service-canary

# 方式 B：给已有实例打 canary 标签（不建议生产使用，但适合快速验证）
consul-cli tag add -service audit-service -tag canary -id <instance-id>
```

`audit-service-canary` 实例启动后必须把自己注册到 Consul，并携带 `canary` 标签。

## Step 2：确认 Consul 已识别 canary

```bash
curl http://localhost:8500/v1/health/service/audit-service?tag=canary
```

应当返回 canary 实例列表。

## Step 3：观察 nginx 配置自动更新

```bash
docker exec observability-consul-template consul-template -config=/etc/consul-template/config.hcl \
  -dry -once 2>&1 | grep -A 5 "audit-service"
```

应当能看到同时出现 `-stable` 与 `-canary` 两个 upstream。

## Step 4：对 canary 做冒烟测试

```bash
# 通过 header 强制路由到 canary
curl -H "X-Canary: true" https://api.example.com/v1/audit/health
# 应当命中 canary 实例

# 不带 header 时，多次请求中约 10% 会命中 canary
for i in {1..100}; do curl -s -o /dev/null -w "%{http_code}\n" https://api.example.com/v1/audit/health; done
```

## Step 5：观测指标

打开 Grafana → Gateway Overview 面板：

- 各服务的请求速率（按 `instance` 标签区分 stable / canary）
- 错误率应当保持平稳
- p95 时延应当与 stable 接近

如果 canary 的错误率突增：

```bash
# 立即回滚 canary
docker compose -f infrastructure/docker-compose-canary.yml down audit-service-canary
# 或
consul-cli tag delete -service audit-service -tag canary -id <instance-id>
```

10 秒内全部流量回切到 stable。

## Step 6：放量到 100%

确认 canary 表现稳定后：

```bash
# 关掉 stable 栈（在 canary 验证成功之后）
docker compose -f infrastructure/docker-compose.yml up -d --scale audit-service-canary=2

# 或：直接用新镜像重新部署 stable
docker compose -f infrastructure/docker-compose.yml up -d audit-service
docker compose -f infrastructure/docker-compose-canary.yml down
```

canary 标签被移除，所有实例都变成 stable。

## 调整流量比例

编辑 `infrastructure/config/nginx/nginx.conf.template`：

```nginx
split_clients $request_id $audit-service_backend {
    50%   audit-service-canary;  # 原本是 10%
    *     audit-service-stable;
}
```

consul-template 会在 ~10 秒内自动 reload nginx。

## 回滚检查清单

- [ ] 从 logs / metrics 定位问题
- [ ] `docker compose -f infrastructure/docker-compose-canary.yml down <svc>-canary`
- [ ] 在 Grafana 上确认 `gateway_request_rate{instance=...canary}` 已归零
- [ ] 24 小时内完成 postmortem，提交 issue / PR

## 暂不支持

- 基于规则的 A/B 测试（需要 service mesh 方案）
- 多区域故障转移
- 粘性会话（仅支持无状态服务）
