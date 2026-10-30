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
├── packages/                       ← 前端 TS 集成包（@digua/contracts + @digua/http）
├── tests/e2e/                      ← Vitest（数据驱动 API 测试）
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
alias cms-openapi='node scripts/dump-openapi-specs.js'
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
| **Pure-admin** | 业务仓前端常用的 Vue 3 管理后台模板（框架已不随附，历史参考） |
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

## 11.5 服务端口速查（Phase 5-A-2 之后）

业务服务直连 HTTP，无 sidecar / 无 mesh。详细架构说明见 [02 架构 §2.1](./02-architecture.md)；服务注册 / 发现见 [`docs-site/guides/service-registration.md`](../guides/service-registration.md)。

| 服务 | 容器内端口 | compose host 端口 | `cargo run` 默认 | Consul catalog | 备注 |
|------|-----------|------------------|-----------------|---------------|------|
| api-gateway | 3000 | 3000 | 3000 | ✅ | 网关入口，绑定 0.0.0.0 |
| iam-service | 8081 | 9081¹ | 8081 | ✅ | ¹ host 端口 9081 是 mesh 时代 sidecar 外端口残留 |
| token-service | 8082 | 8082 | 4000² | ✅ | ² code default 4000，compose 覆盖 8082 |
| notification-service | 8083 | 8083 | 8083 | ✅ | |
| audit-service | 8084 | 8084 | 8080² | ✅ | ² code default 8080，compose 覆盖 8084 |
| prue_backend | 8087 | 8087 | 8080² | ✅ | ² code default 8080，compose 覆盖 8087 |
| file-service | 8082 | 8086³ | 8082 | ✅ | ³ host 8086 消歧（容器内 8082 与 token 共享） |
| config-service | 8081 | 8085³ | 8081 | ✅ | ³ host 8085 消歧（容器内 8081 与 iam 共享） |
| iam-outbox-publisher | — | — | — | ❌ | worker，无 HTTP；不写 `SERVICE_*` labels |
| notification-outbox-publisher | — | — | — | ❌ | worker，无 HTTP |

> 原 frontend 行（容器 80 / host 8080，nginx 反代）已随 PRISM console 移除删除；api-gateway:3000 是唯一公共入口。

> **关键事实**：跨服务调用走 **容器内端口**（`http://iam-service:8081`、`http://token-service:8082`），由 Docker `superpowers` 网络的 DNS 解析；`compose host 端口` 只为宿主机 `curl http://localhost:908x/swagger-ui` 之类开发联调用，gateway / 跨服务都不走 host 端口。

| 基础设施 | 端口 | 协议 | 备注 |
|----------|------|------|------|
| Consul HTTP | 8500 | HTTP | 服务发现 / catalog / DNS |
| Consul Serf LAN | 8301 | TCP/UDP | 集群 gossip |
| Consul gRPC | 8502 | gRPC | |
| registrator | — | — | 项目级注册容器，**挂 `/var/run/docker.sock`**，从 labels 自动注册业务服务（无独立端口） |
| Prometheus | 9090 | HTTP | 指标 |
| Grafana | 3001 | HTTP | admin/admin 首次登录改密 |
| Jaeger UI | 16686 | HTTP | Trace UI |
| OTel collector (OTLP gRPC) | 4317 | gRPC | |
| OTel collector (OTLP HTTP) | 4318 | HTTP | |
| MySQL | 3306 | TCP | |
| Redis | 6379 | TCP | |
| RabbitMQ AMQP | 5672 | TCP | |
| RabbitMQ 管理 UI | 15672 | HTTP | guest/guest |

---

## 11.6 仓库元信息

- **仓库类型**：GitLab（主）+ GitHub（镜像）
- **构建系统**：Cargo workspace
- **Node 工具**：pnpm workspace
- **CI**：GitLab CI（`.gitlab-ci.yml`）
- **Pre-commit**：`.pre-commit-config.yaml`
- **基础镜像**：`zhcoder-docker-registry.com:8000` 内网 registry
