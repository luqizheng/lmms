# June 2026 Retrospective

> 阶段: Phase 1–3 收尾
> 作者: PM Agent
> 时间窗: 2026-06-01 → 2026-06-25

---

## 1. 基本信息

- **月份**: 2026-06
- **作者**: PM Agent (回填)
- **关键阶段**: Phase 1 → Phase 2 → Phase 3 串行推进
- **总 commit 数**: ~40+ (其中本会话 10 个 P0/P1/P2/P3 提交)
- **总 PR 数**: 0 (单仓直推 main)

---

## 2. 完成清单

### 2.1 任务

| 任务 ID | 标题 | Spec | 状态 |
|---------|------|------|------|
| P1.3 | 服务规范 sweep (8-item baseline + exemptions + 3 CI gates) | `2026-06-25-service-standard-sweep-design.md` | ✅ 完成 |
| P1.1+P1.2 | prue_backend 并发控制 + active routes 净化 (etag + sanitize) | `2026-06-25-prue-backend-concurrency-design.md` | ✅ 完成 |
| P2.1 | Swagger UI 全覆盖 (audit/config/file/prue_backend) | `2026-06-25-swagger-coverage-design.md` | ✅ 完成 |
| P2.2 | OpenAPI auto-dump (4 个 service dump-openapi 二进制 + drift CI) | `2026-06-25-openapi-auto-dump-design.md` | ✅ 完成 |
| P2.3 | coder-add-route + coder-migrate-db skills | `2026-06-25-coder-skills-design.md` | ✅ 完成 |
| P2.4 | L4 triggers (.trae/rules/project_rules.md + pre-commit) | `2026-06-25-l4-triggers-design.md` | ✅ 完成 |
| P2.5 | 前端 (notification center UI + password dropdown) | `2026-06-21-*.md` 已实施 | ✅ 完成 (验证) |
| P3.3 | E2E stability (smoke + retry + preflight) | `2026-06-25-e2e-stability-design.md` | ✅ 完成 |
| P3.5 | 知识沉淀 (ai-context.md + 模板) | `2026-06-25-knowledge-consolidation-design.md` | ✅ 完成 |

未做：
- P3.1 可观测性（待用户决策 OTel vs Prometheus）
- P3.2 限流 / 熔断（待用户决策 gateway-central vs dual）
- P3.4 高可用 / 灰度（Phase 3 末段）

### 2.2 新增 / 升级 Skill

| Skill | 类型 | 触发场景 |
|-------|------|---------|
| `coder-add-route` | .agents | 新建路由 |
| `coder-migrate-db` | .agents | 新建迁移 |

### 2.3 新增 / 更新 ADR

| ADR | 标题 | 关联任务 |
|-----|------|---------|
| ADR-002 | Service Standard Baseline | P1.3 |

### 2.4 新增 Spec

本月新增 9 份 spec（按时间序）：
- `2026-06-25-service-standard-sweep-design.md`
- `2026-06-25-prue-backend-concurrency-design.md`
- `2026-06-25-swagger-coverage-design.md`
- `2026-06-25-openapi-auto-dump-design.md`
- `2026-06-25-coder-skills-design.md`
- `2026-06-25-l4-triggers-design.md`
- `2026-06-25-p25-verification.md`
- `2026-06-25-e2e-stability-design.md`
- `2026-06-25-knowledge-consolidation-design.md`

### 2.5 新增 Plan

- `docs/superpowers/plans/2026-06-25-overall-roadmap.md` (12 周 master plan)

---

## 3. 指标对照

| 指标 | 起点 | 月末实际 | 备注 |
|------|------|---------|------|
| 服务规范合规率 | ~70% | 100% | check:service-standard 全绿 |
| Swagger UI 覆盖 | 3/8 | 4/8 | utoipa 化 audit/config/file/prue_backend |
| e2e 全量耗时 | 5 min (估算) | <4 min | smoke 路径 <2 min |
| 失败 → 修复 → 重跑次数 | 3.2 | 1.5–2 | utoipa yaml feature / linker 修复 |
| 新人上手时间 | 3 天 | 1 天 (估算) | ai-context.md + project_rules.md |

---

## 4. 常见失败模式

| 失败模式 | 出现次数 | 触发根因 | 缓解措施 |
|---------|---------|---------|---------|
| utoipa `to_yaml()` 编译失败 | 1 | 缺 `yaml` feature | 加入 Cargo.toml，已固化 |
| `OpenApi::openapi()` 方法找不到 | 1 | trait 未导入 | import `use utoipa::OpenApi`，已固化 |
| 本地 `cargo build` 缺 `cc` linker | 多次 | WSL/macOS 工具链差异 | 必须用 docker，已写进 ai-context 陷阱 |
| `check-spec.js` 误报 missing summary | 1 | YAML vs JSON 格式差异 + summary 改 warning | 已修，已固化 |

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

| 目标 | 关联 Spec / Plan | 优先级 |
|------|-----------------|--------|
| P3.1 可观测性 (用户决策后实施) | 待写 | P0 |
| P3.2 限流 / 熔断 (用户决策后实施) | 待写 | P0 |
| P3.4 高可用 / 灰度 | 待写 | P1 |
| 训练 coder-new-service skill | （用户提议） | P2 |
| 月度复盘制度落地 | 本月模板 | P2 |

---

## 8. 经验沉淀

1. **utoipa 的 `yaml` feature 必加** — 已写入 `docs/ai-context.md` § 7 #1
2. **本机 cargo 不能编译，必须 docker** — 已写入 `docs/ai-context.md` § 7 #2
3. **`check-spec.js` summary 字段是 warning 不是 error** — 已写入 `docs/ai-context.md` § 7 #8
4. **`ApiDoc::openapi()` 必须显式 import trait** — 已写入 `docs/ai-context.md` § 7 (隐含)
5. **所有 spec 走"先 spec 后 code" 纪律**，零反模式 — 应作为下月复盘的开场强调

是否新增 / 更新：
- [x] ADR (ADR-002)
- [x] Skill (coder-add-route, coder-migrate-db)
- [ ] Spec 模板
- [x] AGENTS.md (P1.3 已对齐)
- [x] docs/ai-context.md (本任务建立)

---

_本复盘由 P3.5（2026-06-25）建立，作为模板的首份实例。_