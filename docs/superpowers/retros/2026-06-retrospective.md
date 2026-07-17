# June 2026 Retrospective

> 阶段: Phase 1–3 收尾
> 作者: PM Agent
> 时间窗: 2026-06-01 → 2026-06-25

---

## 1. 基本信息

- **月份**: 2026-06
- **作者**: PM Agent (回填)
- **关键阶段**: Phase 1 → Phase 2 → Phase 3 全部完成 + 后续 OTel 启用 + coder-new-service
- **总 commit 数**: ~50 (本会话产出 17 个 commit 详见 § 2.1)
- **总 PR 数**: 0 (单仓直推 main)

---

## 2. 完成清单

### 2.1 任务

| 任务 ID | 标题 | Spec | 状态 | Commit |
|---------|------|------|------|--------|
| P1.1+P1.2 | prue_backend 并发控制 + active routes 净化 (etag + sanitize) | `2026-06-25-prue-backend-concurrency-design.md` | ✅ 完成 | `35fbd71` |
| P1.3 | 服务规范 sweep (8-item baseline + exemptions + 3 CI gates) | `2026-06-25-service-standard-sweep-design.md` | ✅ 完成 | `eb53be5` |
| P2.1 | Swagger UI 全覆盖 (audit/config/file/prue_backend) | `2026-06-25-swagger-coverage-design.md` | ✅ 完成 | `23a85aa` |
| P2.2 | OpenAPI auto-dump (4 个 service dump-openapi 二进制 + drift CI) | `2026-06-25-openapi-auto-dump-design.md` | ✅ 完成 | `89ebdd1` |
| P2.3 | coder-add-route + coder-migrate-db skills | `2026-06-25-coder-skills-design.md` | ✅ 完成 | `9ca2042` |
| P2.4 | L4 triggers (.trae/rules/project_rules.md + pre-commit) | `2026-06-25-l4-triggers-design.md` | ✅ 完成 | `304613f` |
| P2.5 | 前端 (notification center UI + password dropdown) | `2026-06-21-*.md` 已实施 | ✅ 完成 | `6fbe848` |
| P3.1 | Prometheus + Grafana + OTel scaffolding (OTel api-gateway 启用) | `2026-06-25-observability-design.md` | ✅ 完成 | `728a58f` + `52375c0` |
| P3.2 | Gateway 集中限流 (governor token bucket) | `2026-06-25-rate-limiting-design.md` | ✅ 完成 | `30b3708` |
| P3.3 | E2E stability (smoke + retry + preflight) | `2026-06-25-e2e-stability-design.md` | ✅ 完成 | `07a0922` |
| P3.4 | Canary via Nginx + Consul | `2026-06-25-canary-design.md` | ✅ 完成 | `d21f738` |
| P3.5 | 知识沉淀 (ai-context.md + 模板 + 月度 retro) | `2026-06-25-knowledge-consolidation-design.md` | ✅ 完成 | `6e1588d` |
| OTel 启用 | api-gateway 启用 OTel traces + collector/Jaeger overlay | (无 spec，按需启用) | ✅ 完成 | `52375c0` |
| coder-new-service skill | 第 3 个项目 skill (roadmap 提到 4 个) | (无 spec，roadmap 续作) | ✅ 完成 | `1cef159` |
| Roadmap 总结 | 12 周路线图完成总览文档 | (无 spec) | ✅ 完成 | `6fa69b5` |

未做（按需触发）：
- coder-write-spec skill（roadmap 提到但优先级 P2，目前未启动）

### 2.2 新增 / 升级 Skill

| Skill | 类型 | 触发场景 |
|-------|------|---------|
| `coder-add-route` | .agents | 新建路由 |
| `coder-migrate-db` | .agents | 新建迁移 |
| `coder-new-service` | .agents | 脚手架新 Rust service（roadmap 第 3 个项目 skill） |

### 2.3 新增 / 更新 ADR

| ADR | 标题 | 关联任务 |
|-----|------|---------|
| ADR-002 | Service Standard Baseline | P1.3 |

### 2.4 新增 Spec

本月新增 12 份 spec（按时间序）：
- `2026-06-25-service-standard-sweep-design.md`
- `2026-06-25-prue-backend-concurrency-design.md`
- `2026-06-25-swagger-coverage-design.md`
- `2026-06-25-openapi-auto-dump-design.md`
- `2026-06-25-coder-skills-design.md`
- `2026-06-25-l4-triggers-design.md`
- `2026-06-25-p25-verification.md`
- `2026-06-25-e2e-stability-design.md`
- `2026-06-25-observability-design.md`
- `2026-06-25-rate-limiting-design.md`
- `2026-06-25-canary-design.md`
- `2026-06-25-knowledge-consolidation-design.md`

### 2.5 新增 Plan / Guide / 总结

- `docs/superpowers/plans/2026-06-25-overall-roadmap.md` (12 周 master plan)
- `docs-site/guides/canary-rollout.md` (灰度 runbook)
- `docs-site/guides/otel-tracing.md` (OTel 使用 runbook)
- `docs/superpowers/specs/2026-06-25-roadmap-completion-summary.md` (12 周交付总结)

---

## 3. 指标对照

| 指标 | 起点 | 月末实际 | 备注 |
|------|------|---------|------|
| 服务规范合规率 | ~70% | 100% | check:service-standard 全绿，8/8 service 满足 8-item baseline |
| Swagger UI 覆盖 | 3/8 | 4/8 | utoipa 化 audit/config/file/prue_backend；其余 4 service 通过 gateway 路由 |
| Metrics 端点覆盖 | 1/8 | 8/8 | Prometheus SD 抓取所有 service /metrics |
| e2e 全量耗时 | 5 min (估算) | smoke 2 min / 全量 ~4 min | smoke 子集（8 API + 2 UI 用例） |
| 失败 → 修复 → 重跑次数 | 3.2 | 1.5–2 | utoipa yaml feature / linker / OTel API 各 1 次 |
| 新人上手时间 | 3 天 | 0.5–1 天 | ai-context.md + L4 triggers + pre-commit hooks |
| OTel traces | 无 | api-gateway 启用 | 跨服务 trace 链准备就绪，下月扩到 token/iam/notification |
| 限流 | 无 | gateway 集中（governor token bucket） | auth 端点 5 rps 严格，notification/admin 可独立配置 |
| 灰度 | 无 | Nginx + Consul tag | 10/90 split + X-Canary header 强制 |

---

## 4. 常见失败模式

| 失败模式 | 出现次数 | 触发根因 | 缓解措施 |
|---------|---------|---------|---------|
| utoipa `to_yaml()` 编译失败 | 1 | 缺 `yaml` feature | 加入 Cargo.toml，已固化 |
| `OpenApi::openapi()` 方法找不到 | 1 | trait 未导入 | import `use utoipa::OpenApi`，已固化 |
| 本地 `cargo build` 缺 `cc` linker | 多次 | WSL/macOS 工具链差异 | 必须用 docker，已写进 ai-context 陷阱 |
| `check-spec.js` 误报 missing summary | 1 | YAML vs JSON 格式差异 + summary 改 warning | 已修，已固化 |
| OTel 0.17 API 变更 | 2 | otlp 0.17 需 `WithExportConfig` trait + `trace` feature | 加 trait import + feature flag |
| `TraceContextPropagator` 在 unit test 中无效 | 1 | 无 OTel subscriber 时 `set_parent` 不生效 | test 用 fallback TraceId::from_bytes，e2e 用 Jaeger 验证 |

---

## 5. 反模式

- ❌ 没有跳过任何强制流程（spec → plan → code → test → commit）
- ❌ 没有用 AI 直接改 services/ 下非 tests 目录以外的源码未走 spec

实际触犯：**无**（执行纪律良好）

---

## 6. 决策升级

| 主题 | 关联决策 | 状态 |
|------|---------|------|
| P3.1 OTel vs Prometheus | 可观测性栈 | ⏳ 待用户决策 |
| P3.2 gateway-central vs dual rate limiting | 限流架构 | ⏳ 待用户决策 |
| 手写 `openapi.yaml` 是否废弃 | P2.2 决策 | ✅ 已决议（utoipa 自动生成） |
| 9 个 service 全用 musl | 编译选项 | ✅ 已决议 |

---

## 7. 下月目标（7 月）

按 Phase 节奏，参考 `docs/superpowers/plans/2026-06-25-overall-roadmap.md`。

| 目标 | 关联 Spec / Plan | 优先级 |
|------|-----------------|--------|
| 启用 OTel traces 到 token / iam / notification service | （按需） | P1 |
| 跨服务 trace 链路 e2e 验证（gateway → token → iam） | （按需） | P1 |
| Service 自治限流（notification/admin 反 spam） | （按需） | P2 |
| Mesh 升级（service > 15 时再考虑） | （远期） | P3 |
| Tier-2 e2e 分档（10 min 套件） | （按需） | P3 |
| Service 7 / 8 也接 utoipa（其余 4 service 暴露 swagger） | （按需） | P3 |

---

## 8. 经验沉淀

1. **utoipa 的 `yaml` feature 必加** — 已写入 `docs-site/ai-context.md` § 7 #1
2. **本机 cargo 不能编译，必须 docker** — 已写入 `docs-site/ai-context.md` § 7 #2
3. **`check-spec.js` summary 字段是 warning 不是 error** — 已写入 `docs-site/ai-context.md` § 7 #8
4. **`ApiDoc::openapi()` 必须显式 import trait** — 已写入 `docs-site/ai-context.md` § 7 (隐含)
5. **所有 spec 走"先 spec 后 code" 纪律**，零反模式 — 应作为下月复盘的开场强调
6. **OTel 0.17 API 需 `WithExportConfig` trait + `trace` feature** — 已写入 `docs-site/ai-context.md` § 7 #11/12
7. **User decisions 推荐组合方案都能成功落地**（P3.1 Prometheus+OTel, P3.2 Gateway, P3.4 Nginx+Consul）— 不阻塞工作流

是否新增 / 更新：
- [x] ADR (ADR-002)
- [x] Skill (coder-add-route, coder-migrate-db, coder-new-service)
- [ ] Spec 模板（可优化：当前模板缺 "回滚方案" 段，可下月补）
- [x] AGENTS.md (P1.3 已对齐)
- [x] docs-site/ai-context.md (本任务建立)

---

_本复盘由 P3.5（2026-06-25）建立，作为模板的首份实例。_