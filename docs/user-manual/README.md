# coder-micro-services 用户手册

> 运行、开发、部署、运维 coder-micro-services 平台的完整指南。
> 从这里开始任何任务。
>
> 上次更新：2026-07-06（从原 `docs/USER_MANUAL.md` 按章节拆分并重写）

---

## 目录

| # | 章节 | 速查 |
|---|------|------|
| 01 | [快速上手](./01-quick-start.md) | 环境准备、启动基础设施、跑一个服务、跑 E2E |
| 02 | [架构总览](./02-architecture.md) | 服务地图、分层、典型请求链路、8 项服务标准 |
| 03 | [本地开发](./03-local-development.md) | 本地起服务、热重载路由、调试、加依赖、改契约 |
| 04 | [测试](./04-testing.md) | 测试金字塔、单服务测试、E2E 分层、新增用例 |
| 05 | [可观测性](./05-observability.md) | 链路追踪、指标查询、日志 |
| 06 | [部署](./06-deployment.md) | 构建镜像、CI 流水线、Promote 到生产 |
| 07 | [限流](./07-rate-limiting.md) | 网关 + 服务侧双层限流 |
| 08 | [灰度发布](./08-canary-rollout.md) | Consul + Nginx 切流、回滚 |
| 09 | [AI 协作工作流](./09-ai-workflows.md) | Skills、L4 触发器、Spec 纪律、Pre-commit |
| 10 | [故障排查](./10-troubleshooting.md) | 常见错误速查表、整体重置、按 trace 排查 |
| 11 | [参考](./11-reference.md) | 关键文档、目录布局、命令别名、术语表 |

---

## 面向不同角色

| 你是 | 推荐阅读顺序 |
|------|--------------|
| 新人 / 新 AI 实例 | 01 → 02 → 03 → 04 → 09 |
| 后端开发 | 01 → 02 → 03 → 04 → 09 |
| 运维 / SRE | 01 → 05 → 06 → 07 → 08 → 10 |
| AI Agent | 09 + `docs/ai-context.md` |
| 安全 / 审计 | 02 → 07 → 10 |

---

## 相关文档

- [`docs/ai-context.md`](../ai-context.md) — AI 入口（服务地图、坑位、命令清单）
- [`docs/onboarding/`](../onboarding/) — 中文新手指引（覆盖本地起步、框架、迁移、前端、排查）
- [`docs/decisions/`](../decisions/) — ADR 架构决策记录
- [`docs/guides/`](../guides/) — 专题指南（Canary、OTel、Testing、Java 集成等）
- [`docs/superpowers/specs/`](../superpowers/specs/) — 设计 spec（按 `YYYY-MM-DD-<topic>-design.md` 命名）
- [`docs/superpowers/plans/`](../superpowers/plans/) — 阶段计划
- [`docs/superpowers/retros/`](../superpowers/retros/) — 月度复盘

---

> 本手册与代码库同步演进。结构与章节内容若与最新代码冲突，以代码为准并提 PR 修订。
