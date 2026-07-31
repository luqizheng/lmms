# 11. 参考

> 关键文档、目录布局、命令别名、术语表。

---

## 11.1 关键文档

| 文档 | 用途 |
|------|------|
| [`docs-site/ai-context.md`](../ai-context.md) | AI 入口 — 服务地图、坑位、命令清单 |
| [`docs-site/decisions/ADR-*.md`](../decisions/) | 架构决策记录 |
| [`docs-site/guides/canary-rollout.md`](../guides/canary-rollout.md) | 灰度发布 runbook |
| [`docs-site/guides/otel-tracing.md`](../guides/otel-tracing.md) | OpenTelemetry 接入 |
| [`docs-site/guides/testing.md`](../guides/testing.md) | E2E 数据驱动格式 |
| [`docs-site/guides/observability.md`](../guides/observability.md) | 可观测性专题 |
| [`docs-site/guides/java-integration.md`](../guides/java-integration.md) | Java 端接入 |
| [`docs/superpowers/plans/`](../superpowers/plans/) | 阶段计划（12 周路线图等） |
| [`docs/superpowers/specs/`](../superpowers/specs/) | 设计 spec（每个变更一个） |
| [`docs/superpowers/retros/`](../superpowers/retros/) | 月度复盘 |
| [`docs/onboarding/`](../onboarding/) | 中文新手指引（9 章 + 附录） |
| [`services/<svc>/AGENTS.md`](../../services/) | 单服务约定 |

---

## 11.2 目录布局

```
coder-micro-services/
├── README.md                       ← 项目入口
├── AGENTS.md                       ← 全局 AI 规则
├── Cargo.toml                      ← workspace 根
├── infrastructure/                 ← docker compose + 配置
│   ├── docker-compose.yml          ← 主基础设施
│   ├── docker-compose-observability.yml  ← 可观测性 overlay
│   ├── docker-compose-canary.yml   ← 金丝雀 compose
│   └── config/
│       ├── prometheus/
│       ├── grafana/
│       ├── nginx/
│       ├── otel/
│       └── api-gateway/
│           ├── routes/*.router.toml  ← API 路由
│           └── rate-limits.toml       ← 限流配置
├── services/                       ← 8 个 Rust 服务
│   └── <svc>/
│       ├── Cargo.toml
│       ├── AGENTS.md
│       ├── Dockerfile
│       ├── openapi.yaml            ← 自动生成
│       ├── migrations/
│       └── src/
├── libs/                           ← 共享 Rust crate
│   └── digua_common-telemetry/           ← OTel 脚手架
├── frontend/                       ← Vue 3 管理后台
├── tests/e2e/                      ← Vitest + Playwright
├── contracts/                      ← 单一真相源（YAML）
├── docs/                           ← 本手册 + 指南 + 复盘
│   ├── user-manual/                ← ← 当前手册（拆分版）
│   ├── onboarding/                 ← 中文新手指引
│   ├── guides/                     ← 专题指南
│   ├── superpowers/{plans,specs,retros}/
│   └── decisions/                  ← ADR
├── scripts/                        ← JS 自动化
│   ├── check-service-standard.js
│   ├── check-spec.js
│   ├── contracts-gen.js
│   ├── run-tests-docker.js
│   └── dump-openapi-specs.sh
├── .agents/skills/                 ← AI Skill（项目）
└── .trae/                          ← AI 规则 + 平台 Skill
```

---

## 11.3 命令别名

加到 shell rc：

```bash
alias cms-ci='node scripts/check-service-standard.js && node scripts/check-spec.js && node scripts/contracts-gen.js --check'
alias cms-openapi='scripts/dump-openapi-specs.sh'
alias cms-test='node scripts/run-tests-docker.js'
alias cms-obs='docker compose -f infrastructure/docker-compose.yml up -d'
alias cms-e2e='cd tests/e2e && pnpm test:smoke'
```

---

## 11.4 术语表

| 术语 | 含义 |
|------|------|
| **Consul** | 服务发现 + 健康检查 |
| **JWT** | JSON Web Token（RS256 签名） |
| **OTel / OTLP** | OpenTelemetry / OpenTelemetry Protocol |
| **Pure-admin** | 项目 fork 的 Vue 3 管理后台模板 |
| **prue_backend** | Pure-admin 菜单配置后端（Rust 重写） |
| **smoke subset** | 关键路径冒烟测试子集（快速反馈） |
| **trace_id** | 128-bit ID，经 W3C `traceparent` 透传 |
| **WS / WebSocket** | 双向长连接，用于推送通知 |
| **ADR** | Architecture Decision Record（架构决策记录） |
| **Spec** | `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md` |
| **canary** | 金丝雀实例，承接小比例流量验证新版本 |
| **split_clients** | Nginx 指令，按 key 把流量按比例分发 |
| **fail-open** | 依赖挂了 → 放行（只警告） |
| **forward-only migration** | 迁移只往前走，不修改已发布的版本 |

---

## 11.5 服务端口速查（Phase 5-A）

> mesh 服务有**两个端口**：内端口（`127.0.0.1:180xx`，仅本 namespace）+ 外端口（`0.0.0.0:908x` 或 `3000`，mTLS 入口）。详见 [02 架构 §2.1 / §2.6](./02-architecture.md)。

| 服务 | 内端口（compose） | 外端口（compose / host） | `cargo run` 默认端口 | mesh |
|------|------------------|--------------------------|--------------------|------|
| api-gateway | 18000 | 3000 | 3000 | ✅ |
| iam-service | 18081 | 9081 | 8080 | ✅ |
| token-service | 18082 | 9082 | 8080 | ✅ |
| notification-service | 18083 | 9083 | 8080 | ✅ |
| audit-service | 18084 | 9084 | 8080 | ✅ |
| prue_backend | 18087 | 9087 | 8080 | ✅ |
| file-service | — | 8082 | 8082 | ❌ |
| config-service | — | 8081 | 8081 | ❌ |
| frontend | — | 8080 → 80 | — | — |

| 基础设施 | 端口 | 协议 |
|----------|------|------|
| Nginx | 80 | HTTP |
| Consul HTTP | 8500 | HTTP |
| Consul Serf LAN | 8301 | TCP/UDP |
| Consul Connect CA RPC | 8303 | TCP |
| Consul Envoy stats | 8502 | HTTP |
| Prometheus | 9090 | HTTP |
| Grafana | 3001 | HTTP |
| Jaeger UI | 16686 | HTTP |
| OTel collector (OTLP gRPC) | 4317 | gRPC |
| MySQL | 3306 | TCP |
| Redis | 6379 | TCP |
| RabbitMQ AMQP | 5672 | TCP |
| RabbitMQ 管理 UI | 15672 | HTTP |

---

## 11.6 仓库元信息

- **仓库类型**：GitLab（主）+ GitHub（镜像）
- **构建系统**：Cargo workspace
- **Node 工具**：pnpm workspace
- **CI**：GitLab CI（`.gitlab-ci.yml`）
- **Pre-commit**：`.pre-commit-config.yaml`
- **基础镜像**：`zhcoder-docker-registry.com:8000` 内网 registry
