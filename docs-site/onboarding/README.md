# 新人入职手册

> 本文档面向刚入职、需要快速上手本项目的程序员（前后端均覆盖）。
> 不用提前会 Rust / Vue，跟着步骤走就能跑起来。
>
> **建议阅读顺序**：01 → 02 → 03 为必读主线，04–09 按需翻阅。

---

## 章节地图

| 章节 | 标题 | 一句话 | 推荐阅读时机 |
|------|------|--------|-------------|
| 01 | 项目概览 | 本项目是什么、有哪些服务、用什么技术栈 | 入职第 1 天 |
| 02 | 环境准备 | 本机要装什么、怎么验证装对了 | 入职第 1 天 |
| 03 | 第一次跑通 | 从 clone 到看到第一个接口响应 | 入职第 1–2 天 |
| 04 | 后端框架与配置 | 每个框架做什么、配置文件每个字段含义 | 入职第 2–3 天 |
| 05 | 数据库与迁移 | 本地数据库怎么连、迁移怎么加 | 入职第 2–3 天 |
| 06 | 前端开发 | Vue 3 栈怎么起、怎么与后端联调 | 入职第 2–3 天（前端） |
| 07 | 日志与排查 Bug | 服务报错时去哪看、怎么跨服务串联一次请求 | **随时查阅** |
| 08 | 常见任务速查 | 加接口、加表、跑测试、改路由等入口 | 日常速查 |
| 09 | 出问题怎么办 | 卡住时的自检清单与 bug 报告模板 | 卡住时 |
| 附录 | 术语与链接 | 外部链接汇总表 + 术语表 | 按需 |

---

## 在哪里查全功能参考

本文档是**线性入门引导**，深度专题请跳转到已有参考文档：

| 主题 | 文档 |
|------|------|
| 全功能参考手册（中文，按章节拆分） | [`docs/user-manual/`](../user-manual/) |
| API Gateway 路由配置 | [`docs-site/guides/api-gateway-routing.md`](../guides/api-gateway-routing.md) |
| OpenTelemetry 跨服务追踪 | [`docs-site/guides/otel-tracing.md`](../guides/otel-tracing.md) |
| 可观测性（Prometheus + Grafana + Jaeger） | [`docs-site/guides/observability.md`](../guides/observability.md) |
| 灰度发布 | [`docs-site/guides/canary-rollout.md`](../guides/canary-rollout.md) |
| E2E 测试（数据驱动格式） | [`docs-site/guides/testing.md`](../guides/testing.md) |

---

## 反馈与更新

本文档与代码库同步维护。如发现命令过期或步骤不通，请按照文末 bug 报告模板提 issue。

---

## 日常活动总图（入职后用）

通读完 01–09 入门后，日常开发工作（拿任务 → 改 → 测 → 推 → 出镜像 → 部署 → 观测）的**统一动作链**见：

**[`../developer-workflow.md`](../developer-workflow.md)**

> 一份图看完整个开发者动作链，按命令 + 链接索引到详细文档。日常速查、推送前自检、出镜像流程、灰度发布 SOP 均在此文件中，工作时按需跳转。

onboarding 是**课程**（一次通读），`developer-workflow.md` 是**地图**（持续翻阅），两者互补。
