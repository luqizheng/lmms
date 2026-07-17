# P1.3 服务规范 Sweep 设计

> 日期：2026-06-25
> 范围：8 个服务（api-gateway / token-service / iam-service / notification-service / audit-service / config-service / file-service / prue_backend）+ `scripts/check-service-standard.js` + `services/agents-template.md` + `.gitlab-ci.yml`
> 状态：待用户审阅
> 关联：[overall-roadmap](../plans/2026-06-25-overall-roadmap.md) P1.3

---

## 1. 目标

让 8 个服务全部对齐 `services/agents-template.md` 定义的规范基线，`pnpm run check:service-standard` 0 warning、`pnpm run check:spec` 0 error、`contracts-gen` 零漂移，并将这三项作为 CI 强制闸门。

完成标志：

1. 8 个服务的 `AGENTS.md` 表格一致、`必需组件` checklist 全部齐备
2. `check-service-standard.js` 规则集与模板逐项对齐，且能识别**所有**模板要求项
3. `.gitlab-ci.yml` 在 test 阶段串联 3 个强制检查（规范 / 契约规范 / 契约漂移）
4. ADR-002 落地，固化基线，作为后续新服务的硬约束

---

## 2. 当前状态盘点（事实，非推断）

### 2.1 服务文件存在性

| 服务 | Dockerfile | AGENTS.md | config/ | migrations/ | README.md | tests/ | openapi.yaml |
|---|---|---|---|---|---|---|---|
| api-gateway | ✓（+Dockerfile.local） | ✓（小写 agents.md）| ✗ | ✗（无 DB） | ✓ | ✓ | ✗ |
| token-service | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| iam-service | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| notification-service | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| audit-service | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| config-service | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| file-service | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| prue_backend | ✓ | ✓ | ✗ | ✓ | ✓ | ✓ | ✗ |

> 备注：api-gateway 文件名为 `agents.md`（小写），与其它 7 个 `AGENTS.md`（大写）不一致 → 需统一为 `AGENTS.md`。

### 2.2 端点接入

| 服务 | /health | /metrics | SwaggerUi |
|---|---|---|---|
| api-gateway | ✓（lib.rs/main.rs） | ✓ | utoipa imported, no SwaggerUi mount |
| token-service | ✓ | ✓ | ✓ |
| iam-service | ✓ | ✓ | ✓ |
| notification-service | ✓ | ✓ | ✓ |
| audit-service | ✓ | ✗ | ✗ |
| config-service | ✗ | ✗ | ✗ |
| file-service | ✓ | ✓ | ✗ |
| prue_backend | ✓ | ✓ | ✗ |

> 备注：config-service 未挂 `/health` 或 `/metrics`；audit-service 只有 `/health`；4 个服务（api-gateway / audit / config / file / prue）未挂 SwaggerUi。

### 2.3 工具脚本现状

- `scripts/check-service-standard.js`：仅识别 6 项（Dockerfile / Swagger 配置 / 健康检查端点 / 数据库迁移文件 / README.md / tests/ 目录），且把"健康检查端点"判定为 `src/health.rs` 文件存在——实际项目里 `/health` 是在 `routes/mod.rs` 或 `main.rs` 注册的，规则不准
- `services/agents-template.md` 模板规定 6 项必需组件 + 3 个环境变量开关（SWAGGER_ENABLED / HEALTH_CHECK_REQUIRED / MIGRATIONS_REQUIRED）
- 各服务 AGENTS.md 实际**没有完整复用模板**（如 prue_backend 自定义了端点表、token-service 自带 FEATURES.md）

### 2.4 历史日志污染

根目录有 `build-gw.log`（41 KB）和 `build-iam.log`（50 KB），未纳入 `.gitignore`。

---

## 3. 规范基线（统一版）

### 3.1 必需组件（8 项）

| 项 | 判定方式 |
|---|---|
| Dockerfile | 文件存在 |
| AGENTS.md | 文件存在且大小写为 `AGENTS.md` |
| README.md | 文件存在且包含「运行 / 配置 / 端点」三节 |
| tests/ 目录 | 目录存在且至少包含 1 个 `.rs` 文件 |
| /health 端点 | `routes/mod.rs` 或 `main.rs` 注册 `.route("/health", ...)` |
| /metrics 端点 | 同上注册 `.route("/metrics", ...)` |
| migrations/ 目录 | 目录存在（无 DB 服务可豁免，见 3.3） |
| Swagger 集成 | `Cargo.toml` 含 `utoipa-swagger-ui` 依赖 + `main.rs` 含 `SWAGGER_ENABLED` 开关（可豁免，见 3.3） |

### 3.2 标准环境变量（AGENTS.md 表格统一字段）

| 变量 | 默认 | 说明 |
|---|---|---|
| `SERVER_HOST` | `0.0.0.0` | 监听主机 |
| `SERVER_PORT` | `8080` | 监听端口 |
| `SERVER_ADDR` | `0.0.0.0:8080` | 全地址（优先级高于上两项）|
| `DATABASE_URL` | 服务自定义 | 数据库连接 |
| `RUST_LOG` | `info` | 日志级别 |
| `SWAGGER_ENABLED` | `false` | 是否挂 SwaggerUi |
| `CONSUL_ADDR` | 服务自定义 | Consul 地址 |

### 3.3 豁免规则

- **api-gateway**：无业务 DB → 豁免 `migrations/`；但仍需保留 SwaggerUi（聚合视图，Phase 2 P2.1 完成）
- **token-service / iam-service / notification-service / audit-service / config-service / file-service / prue_backend**：均接 DB → 必须有 `migrations/`
- 任何豁免必须在 `AGENTS.md` 的「豁免」段显式声明

---

## 4. 改造范围

### 4.1 服务级（按顺序，每服务独立 commit）

执行顺序：先小后大，便于早暴露规则问题

1. **audit-service** — 补 `/metrics`、SwaggerUi 开关、AGENTS.md 字段对齐
2. **config-service** — 补 `/health` + `/metrics`、SwaggerUi 开关、AGENTS.md 对齐
3. **file-service** — 补 SwaggerUi 开关、AGENTS.md 对齐
4. **prue_backend** — AGENTS.md 对齐（保留服务专属的端点表，字段名与 3.2 表统一）、补 SwaggerUi 开关
5. **notification-service** — AGENTS.md 字段对齐（已有 Swagger）
6. **token-service** — AGENTS.md 字段对齐（已有 Swagger）
7. **iam-service** — AGENTS.md 字段对齐（已有 Swagger）
8. **api-gateway** — 文件名 `agents.md` → `AGENTS.md`、SwaggerUi 留作 P2.1（本阶段只声明豁免）

每服务子任务：

- [a] 重写 `AGENTS.md`：必需组件 8 项 + 环境变量表统一字段 + 豁免段（若有）
- [b] 补 `/health` 或 `/metrics`（仅 audit / config）
- [c] 接 `SWAGGER_ENABLED` 开关 + `utoipa-swagger-ui` 依赖（audit / config / file / prue）
- [d] 改完跑 `node scripts/run-tests-docker.js <service>` 通过

### 4.2 模板与工具

- **`services/agents-template.md`**：升级为完整模板（含端点表 / 环境变量表 / 豁免段 / AI 检查时机）
- **`scripts/check-service-standard.js`**：
  - 必需组件识别改为查 8 项（见 3.1）
  - "健康检查端点" 改为读 `src/main.rs` + `src/routes/mod.rs` 的 grep（regex：`\.route\("/health"`）
  - 支持「豁免」段：若 AGENTS.md 含 `## 豁免\n- <item>`，则该项跳过
  - 输出退出码：有 warning → exit 1（CI 闸门）
- **`.gitignore`**：新增 `build-*.log`（根目录历史日志）；删除已存在的 `build-gw.log` / `build-iam.log`

### 4.3 契约一致性闸门

- `scripts/contracts-gen.js`：增加 `--check` 子命令，等价于"gen 后 `git diff --exit-code`"，退出码 1 表示漂移
- `package.json`：新增 `contracts:check`（已存在则确认其行为）
- `.gitlab-ci.yml` 的 `test` 阶段顺序串：
  ```yaml
  test:
    script:
      - pnpm run check:service-standard
      - pnpm run check:spec
      - pnpm run contracts:check
      - node scripts/run-tests-docker.js
  ```
  任一红 → 阻断后续 build / deploy

### 4.4 ADR

新增 `docs-site/decisions/ADR-002-service-standard-baseline.md`：

- 上下文：服务长出后规范漂移
- 决议：固定 3.1 的 8 项 + 3.2 的 7 个环境变量 + 3.3 的豁免规则
- 后果：新增服务必须复制 `services/agents-template.md` + CI 闸门强制守护

---

## 5. 数据流 / 调用关系（无运行时改变）

P1.3 不引入运行时新逻辑，只调整：

- 端点注册（audit / config 补 `/metrics`，config 补 `/health`）：纯 axum router 加路由，复用已有 `health()` / `metrics()` handler 模式
- SwaggerUi 挂载：复用 iam-service / notification-service 中已实证的 4 行接入代码
- 工具脚本：新增/调整规则识别，不动业务

CI 流变化：

```
push → GitLab Runner
       └─ test stage
          ├─ check:service-standard   ← 新闸门
          ├─ check:spec               ← 新闸门
          ├─ contracts:check          ← 新闸门
          └─ run-tests-docker         ← 已有
       └─ build stage（test 红则跳过）
       └─ deploy stage（build 红则跳过）
```

---

## 6. 错误处理

| 场景 | 处理 |
|---|---|
| 服务 AGENTS.md 缺失 | `check-service-standard.js` 输出 `⚠️  缺失规则文件: AGENTS.md` + exit 1 |
| 必需组件缺失 | 输出 `⚠️  缺失: <项>` + exit 1 |
| 豁免段格式错误 | 视为无豁免，正常判断；不静默通过 |
| 契约漂移 | `contracts-gen --check` 输出 git diff + exit 1，提示「请提交生成产物」 |
| Swagger build.rs 联网失败 | 已用 `gh-proxy.org` 镜像（agents.md 网络规则）；CI 容器 `--network host` |

---

## 7. 测试策略

### 7.1 单元

- `scripts/check-service-standard.js`：跑两次
  - 当前仓库（修复后）→ 0 warning
  - 故意删 `services/audit-service/README.md` → exit 1，输出含 `缺失: README.md`
- `scripts/contracts-gen.js --check`：跑两次
  - 当前仓库 → exit 0
  - 故意改 `contracts/error-codes.yaml` 不重新 gen → exit 1

### 7.2 集成

- 每服务过 `node scripts/run-tests-docker.js <service>` 一次（共 8 次，CI 已有 `run-tests-docker.js` 自动遍历）
- 全量 e2e：`pnpm run test:e2e` 至少一次零失败

### 7.3 验收 Demo

```bash
pnpm run check:service-standard   # 期望 exit 0
pnpm run check:spec               # 期望 exit 0
pnpm run contracts:check          # 期望 exit 0
git diff --exit-code              # 期望 exit 0
node scripts/run-tests-docker.js  # 期望全服务 PASS
pnpm run test:e2e                 # 期望全套 PASS
```

---

## 8. 工作量预估

| Step | 工作量 |
|---|---|
| Step 1 盘点 + 输出差距表 | 0.5 天 |
| Step 2 模板与脚本对齐（agents-template + check-service-standard） | 0.5 天 |
| Step 3 8 个服务收口 | 4–7 天（0.5–1 天/服务） |
| Step 4 contracts-gen --check + .gitlab-ci.yml | 0.5 天 |
| Step 5 ADR-002 + e2e 全量验证 | 0.5 天 |
| **合计** | **6–9 工作日（单人节奏）** |

---

## 9. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 改 8 个服务的 main.rs 撞合并冲突 | 每服务独立 commit + 按顺序串行；不开多个并行 PR |
| `check-service-standard.js` 规则比模板严 / 松 | Step 2 先对齐规则与模板，验证 8 服务全过再继续 |
| Swagger build.rs 联网下载阻塞 | 沿用 `gh-proxy.org` 代理；容器跑测试用 `--network host`（已有） |
| 手改过的契约生成产物回滚丢字段 | Step 4 用 `git blame` 找到首次手改 commit，决定是回写 YAML 还是把改动正式纳入契约 |
| audit / config / file 接 Swagger 触发 utoipa 学习成本 | 直接复制 iam-service / notification-service 的 4 行接入代码作为参考 |
| api-gateway 文件名改大小写在 Linux/Windows 文件系统行为不同 | 用 `git mv -f`，commit message 显式说明 |

---

## 10. 不在范围

- 真实 Prometheus 指标暴露（仅保留 `/metrics` 占位 handler，Phase 3 P3.1 再做）
- api-gateway 聚合 SwaggerUi（Phase 2 P2.1）
- 服务级单元/集成测试质量补强（独立任务，不在 P1.3）
- prue_backend 草稿乐观锁（P1.1）
- active routes `_menu` 净化（P1.2）

---

## 11. 接口契约（无外部 API 变化）

P1.3 不改任何对外 HTTP 接口。`/health` 与 `/metrics` 沿用现有 handler 的响应体（普通 200 + `OK` / Prometheus 文本占位）。新增的 `/swagger-ui` 仅在 `SWAGGER_ENABLED=true` 时挂载，默认关闭。

---

## 12. 验收清单

- [ ] 8 个服务 `AGENTS.md` 全部对齐 3.1 的 8 项 + 3.2 的环境变量表
- [ ] api-gateway 文件名从 `agents.md` 改为 `AGENTS.md`
- [ ] audit-service 补 `/metrics` + SwaggerUi 开关
- [ ] config-service 补 `/health` + `/metrics` + SwaggerUi 开关
- [ ] file-service 补 SwaggerUi 开关
- [ ] prue_backend 补 SwaggerUi 开关
- [ ] `services/agents-template.md` 升级为完整模板
- [ ] `scripts/check-service-standard.js` 升级为 8 项规则 + 豁免段 + exit code
- [ ] `scripts/contracts-gen.js --check` 子命令落地
- [ ] `.gitlab-ci.yml` test 阶段串 3 闸门
- [ ] `.gitignore` 加 `build-*.log`，删除根目录两个历史日志
- [ ] `docs-site/decisions/ADR-002-service-standard-baseline.md` 提交
- [ ] 所有验收 Demo 命令 exit 0

---

## 13. 关联文档

- 总路线图：[2026-06-25-overall-roadmap.md](../plans/2026-06-25-overall-roadmap.md)
- 模板：[services/agents-template.md](../../../services/agents-template.md)
- 现有规则：[agents.md 服务规范规则系统段](../../../agents.md)
- 契约：[contracts/](../../../contracts/)
- 长期 AI 协同：[2026-06-09-long-term-ai-efficiency-plan.md](../../plans/2026-06-09-long-term-ai-efficiency-plan.md)
