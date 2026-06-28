# 51 — 韧性三件套（超时/重试/熔断）手工验收

> 关联：`docs/superpowers/specs/2026-06-28-phase5-resilience-triad-design.md`
> 前置：`docker compose` 起 gateway + 至少一个下游（以 audit-service 为例）+ Consul。
> 默认配置：timeout=3s, overall=8s, breaker_threshold=10, open_timeout=30s, half_open_max=3。

## 场景 1：熔断打开 → 短路 503

| 步骤 | 操作 | 期望 |
|------|------|------|
| 1 | `docker compose stop audit-service`（令其拒连） | 下游不可用 |
| 2 | 连续 ≥10 次 `curl -i http://localhost:3000/api/v1/audit/health` | 前若干次 502/504，累积达阈值后转 503 |
| 3 | 再次 `curl -i .../audit/health` | **503** + body `{"error":"circuit_open","service":"audit-service"}`（短路，未打到下游） |
| 4 | `curl -s http://localhost:3000/metrics \| grep gateway_circuit_state` | `gateway_circuit_state{service="audit-service"} 2`（Open） |
| 5 | `curl -s .../metrics \| grep gateway_upstream_requests_total` | 含 `outcome="circuit_open"` 计数 > 0 |

## 场景 2：HalfOpen 探测 → 恢复 Closed

| 步骤 | 操作 | 期望 |
|------|------|------|
| 1 | 承场景 1（breaker 处于 Open） | — |
| 2 | `docker compose start audit-service`，等待其在 Consul 健康 | 下游恢复 |
| 3 | 等待 ≥30s（open_timeout） | breaker 允许进入 HalfOpen |
| 4 | `curl -i .../audit/health` 连续 3 次 | 探测成功，逐步恢复 |
| 5 | `curl -s .../metrics \| grep gateway_circuit_state` | 值回到 `0`（Closed） |

## 场景 3：超时映射 504

| 步骤 | 操作 | 期望 |
|------|------|------|
| 1 | 令下游对某路径响应 > 3s（临时在下游加 sleep handler，或用 tc 注入延迟） | 单次超时 |
| 2 | `curl -i .../<慢路径>` | **504** upstream timeout |
| 3 | 持续慢（每次 > 3s）| 总耗时 ≤ ~8s（overall deadline 封顶），仍 504 |

## 场景 4：幂等重试 vs POST 不重试

| 步骤 | 操作 | 期望 |
|------|------|------|
| 1 | 令下游对 GET 间歇性返回 502（前 1-2 次失败后成功） | — |
| 2 | `curl -i .../audit/operation`（GET）| 经重试后最终 2xx；`gateway_upstream_retries_total` 增加 |
| 3 | 对 POST 路径同样间歇 502 | POST **不重试**，首个 502 即透传 |
