# K3s / Kubernetes 部署决策重审

> 日期：2026-07-29
> 关联文档：
> - [pre-v1-todo.md](../../pre-v1-todo.md) § P0-4, P1-4
> - [ADR-005 Shared-DB vs Per-Service-DB](./decisions/ADR-005-shared-db-vs-per-service-db.md)
> - [ADR-006 Gateway 长期策略](./decisions/ADR-006-gateway-long-term-strategy.md)
> - [性能基线报告](./perf-baseline.md)

## 背景

2026-07-19 决策：Phase 7 从 P2 降级至 P10，理由是"当前无客户使用 K8s"。

2026-07-29 架构评审（pre-v1-todo.md）指出：这个决策只考虑了外部客户，但**内部已在生产上单点跑**（Docker Compose 单机）。K3s 不是给客户看的功能，是给自己的运维可靠性用的。

## 重审结论

### 1.0 阶段：维持 Docker Compose

**决定**：1.0 上线仍以 Docker Compose 为部署方案。理由：

| 因素 | 说明 |
|------|------|
| 交付形态 | 1.0 是离线私有化交付（`release/dist-template/`），消费方期望的是简单的一键部署，不是 K8s 集群 |
| 可靠性已补 | P0-4 备份 Runbook 已覆盖 MySQL/Redis/RabbitMQ/Consul 手动备份恢复 |
| 滚动更新已有 | CI deploy 阶段已是逐服务重启 + healthcheck 门禁（非全停机） |
| 团队带宽 | 1.0 前有 P0 安全项、CI 补齐、观测栈闭环等更高优先级任务 |

### 2.0 阶段：内部环境跑 K3s，外部交付维持 Docker Compose

**规划**：

| 环境 | 部署方案 | 理由 |
|------|---------|------|
| **内部开发 / 测试** | Docker Compose（现状） | 快速启动、调试方便 |
| **内部 SaaS / 公网** | K3s 3 节点集群 | 需要 HA、自动故障转移、资源调度 |
| **外部私有化交付** | Docker Compose（现状） | 客户环境大概率是单机或小规模 |
| **CI/CD 验证环境** | K3s（与内部 SaaS 同环境） | 吃自己的狗粮 |

**2.0 K3s 最小集群规格**：

- 3 个 master/worker 混合节点（资源有限时）
- Consul 3 副本（HA 服务发现）
- MySQL 主从（1 主 1 从，灾备）
- Redis Sentinel（3 节点）
- RabbitMQ mirrored queues
- 各服务 2+ replica（`Deployment` + `HorizontalPodAutoscaler`）

### 触发条件

当以下任一条件满足时，启动 K3s 迁移：

1. 首个内部 SaaS tenant 上线（有外部用户流量）
2. 单机 Docker Compose 撑不住预期流量（参考 `perf-baseline.md` 基线数据）
3. 出现一次因单点故障导致的生产事故（Docker 守护进程挂了 = 全平台挂）

### 为什么不是现在

- 离线交付版本依赖 Docker Compose 的简单性，双轨维护（Compose + K3s）在当前阶段是过度工程
- K3s 的运维成本不是零——需要有人管集群升级、证书轮换、存储卷、网络策略
- P0-4 已覆盖单点故障的手动恢复 SOP，短期风险可控
- 1.0 上线优先级：功能完整 > 基础设施完美

## 行动项

| 状态 | 行动 | 负责人 | 时间 |
|------|------|--------|------|
| ⬜ | 完成首次负载摸底，记录单机瓶颈数据到 `docs-site/perf-baseline.md` | — | 1.0 前 |
| ⬜ | 如果基线数据显示单机撑不住预期流量，触发 K3s 评估 | — | 基于数据 |
| ⬜ | 首个内部 SaaS tenant 上线前，准备 K3s 最小集群部署脚本 | — | 2.0 |
| ⬜ | K3s 就绪后，按 ADR-005 逐服务拆 DB | — | 2.0 |
| ⬜ | K3s 就绪后，按 ADR-006 评估 Envoy 替换网关 | — | 2.0 |

## 参考

- [pre-v1-todo.md](../../pre-v1-todo.md) — 完整 1.0 整改清单
- [backup.md](./runbooks/backup.md) — 单点故障手动恢复 SOP
- [ADR-005](./decisions/ADR-005-shared-db-vs-per-service-db.md) — DB 隔离策略
- [ADR-006](./decisions/ADR-006-gateway-long-term-strategy.md) — 网关长期策略
