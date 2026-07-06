# 07. 限流

> 双层架构：网关（粗）+ 服务侧（细）。

---

## 7.1 为什么是两层

| 层 | 位置 | 算法 | 范围 | 用途 |
|----|------|------|------|------|
| **Layer 1：网关** | api-gateway | 内存 `governor` 令牌桶 | route 级 | 防 DDoS、防爆破、底线公平 |
| **Layer 2：服务侧** | 各业务服务 | Redis 滑动窗口 ZSET | 业务规则 | "管理员每分钟最多发 5 条公告" |

请求**必须过两层**才算放行，任一层失败 → `429 Too Many Requests`。

```
Request
  ↓
[Nginx]
  ↓
[api-gateway]
  ├─ JWT 校验（注入 X-User-Id）
  ├─ 网关限流（route 级、内存）
  ├─ 黑名单检查
  └─ 代理 handler（注入 traceparent）
      ↓
  [下游服务，如 notification-service]
      └─ 服务侧限流（业务规则、Redis）
          ↓
      [业务 handler]
```

---

## 7.2 网关层配置

文件：`infrastructure/config/api-gateway/rate-limits.toml`

```toml
[defaults]
requests_per_second = 100
burst = 200

[routes."/v1/iam/auth/login"]
requests_per_second = 5
burst = 10
```

改完重启 gateway 生效。

**身份解析顺序**（取第一个非空）：

1. `user_id`（来自已校验 JWT）
2. `X-Real-IP`（Nginx 透传）
3. 远端 socket IP

> 内存限流**不跨副本同步**——多实例部署时单实例额度 = 配置值 × 实例数。要严格总额度请走 Layer 2。

---

## 7.3 服务侧配置（notification-service 示例）

`services/notification-service/src/rest.rs`：

```rust
pub fn default_limits() -> Vec<Limit> {
    vec![
        Limit::new("POST /notification/announcements", 5, 60),       // 防刷
        Limit::new("POST /notification/announcements/{id}/publish", 10, 60),
        Limit::new("POST /notification/announcements/{id}/revoke", 10, 60),
    ]
}
```

目前 `notification-service` 是首个接入服务侧限流的服务。其它服务按需 opt-in：

1. `Cargo.toml` 加 `common-ratelimit = { path = "../../libs/common-ratelimit" }`
2. 创建 `RateLimiter::new(redis.conn.clone(), default_limits())`
3. 挂中间件：`.route_layer(axum::middleware::from_fn_with_state(rate_limiter, rate_limit_middleware))`
4. `main.rs` 加 `common_ratelimit::init_metrics("service-name")`

---

## 7.4 限流响应

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 60
Content-Type: application/json

{
  "code": "RATE_LIMITED",
  "message": "too many requests",
  "retry_after_ms": 60000
}
```

客户端应按 `Retry-After` 退避后重试。

---

## 7.5 失败模式

**服务侧限流 fail-open**：Redis 不可达 → 放行（仅记 warn 日志）。

理由：Redis 挂掉不应让整个产品挂掉；网关层（内存）依然能挡住 DDoS。

---

## 7.6 可观测性

| 指标 | 层 | 标签 |
|------|-----|------|
| `gateway_rate_limited_total` | gateway | route |
| `service_rate_limited_total` | service | service, route |

Grafana → **Gateway Overview** → "Rate Limited (req/s by route)" 已展示前者；后者随服务接入数量增加会补独立面板。

---

## 7.7 调优建议

| 场景 | 建议 |
|------|------|
| 公开端点（登录、注册） | 严格网关限流，rate 低、burst 更低 |
| 管理后台操作 | 服务侧限流，按业务规则配 |
| 高 QPS 内部接口 | 网关 `[defaults]` 放大 + 服务侧细粒度 |
| 防爬 | 网关按 IP，服务侧按 user |
