# 脚本层全量收编进 coder CLI 设计（AI 主导执行面）

日期：2026-11-02
状态：draft（brainstorming 产出，用户裁决已并入，待批准）
基线分支：`main`（`fb744f48`）
前序设计：`2026-10-30-coder-cli-engineering-shell-design.md`（本文为其续篇，收敛「全量收编」的实施形态）

## 一、意图与背景

用户裁决：**脚本层全量收编进 CLI**，不采用薄壳包裹或只改 skill 指向。目的是让发布、���发、测试、守卫、运维等全部脚本**服务于 AI 开发**——agent 说意图、编排命令、自校验结果，每一步都不需要读散文猜用法。

### 1.1 现状盘点（2026-11-02 实测）

| 层 | 现状 | 证据 |
|---|---|---|
| 编排层 | 22 个 skill（`.agents/skills` 15 + `.trae/skills` 7），20 个含硬编码 `node scripts/` 路径，共 60 处引用 | `grep -rc 'node scripts/' .agents/skills/*/SKILL.md` |
| 决策层 | `packages/cli` 11 条命令（release 6 / assets 1 / build 1 / doctor 1 / version 1 / compose-template 1），契约骨架已成型 | `packages/cli/lib/commands.js` |
| 执行层 | `scripts/` 38 个 JS（约 8.6k 行）+ `release/dist-template/bin/` 11 个；**仅 1 个支持 `--json`**（`check-java-sdk-drift.js`） | `grep -l json scripts/*.js` |
| 裁判层 | CI 16 job（`.gitlab-ci.yml`）中 9 处调 `node scripts/`；pre-commit 4 个 hook 全部调 `node scripts/` | 实测 grep |

**执行层与编排层之间存在两处断裂**：

1. **编排层 → 执行层断裂**：AI 拿到的 skill 剧本指向旧 `scripts/` 轨，而旧轨 99% 无机读输出、无稳定退出码语义。
2. **裁判层 → 决策层断裂**：CI 与 pre-commit 仍以旧轨为判据，与 CLI 的判据可能不一致，AI 自证结果与 CI 结论可能打架。

**命令面缺口**：`doctor` 自陈 `ops … (batch 2)`；`dev` / `test` / `check` / `init` / `ops` 五条命令面一条都没有。AI 无编排手段，只能拼裸脚本。

### 1.2 已实测的契约缺陷（B0 必修）

| 缺陷 | 实测 | 危害 |
|---|---|---|
| `--help` 在子命令之后被当作 positional 静默吞掉 | `coder assets download --help` **真的执行了下载**，未显示帮助 | agent 探查命令用法时触发真实副作用 |
| 未知 flag 静默通过 | `coder release status --totally-bogus` → exit 0，正常输出信封 | agent 拼错参数名得不到任何反馈 |
| 守卫失败阈值不统一 | `check-spec` 仅硬错误 exit 1、warn 不影响；`check-db-naming` 当前 exit 0；`check-service-standard` 另有自定 `failed` 标记 | AI 无法判定「这次 check 通过了没有」 |

### 1.3 与前序 spec 的关系

`2026-10-30-coder-cli-engineering-shell-design.md` 已定调：单一 CLI 包 `@digua/cli`、命令面按桶组织、`--json` + 稳定退出码、skills 剧本化、入口收敛、`scripts/` 清零。本文**不推翻**其任何裁决，只补齐三处它留给建造者的空白：① 完整命令树与桶内命令清单；② 迁移批次与双轨收敛机制；③ 工具层错误码与守卫失败阈值的单一权威源。

## 二、目标

1. **命令面**：约 62 条命令，七大桶（`check` / `test` / `dev` / `build` / `release` / `init` / `ops`）+ 顶层（`doctor` / `schema` / `version` / `assets`）。覆盖 `scripts/` 38 个 + `dist-template/bin/` 11 个 + 已有 11 条 release 线。
2. **契约一致**：所有命令共享 stdout 单行 JSON 信封、stderr 日志、退出码 0/1/2/3/4、未知 flag 报 `USAGE`、`--help` 任意位置生效。
3. **守卫可判**：每个守卫显式声明失败阈值，error 级 finding → exit 3、warn 级 → exit 0；findings 结构化可机读。
4. **skill 剧本机读化**：22 个 skill 的步骤从散文改为「命令 → 期望退出码 → JSON 字段判据 → 失败处置」四元组。
5. **AI 可发现**：`coder schema --json` 输出全量命令契约；skill 剧本声明 `requires:`，CLI 反向校验剧本引用的命令是否存在，漂移在 CI 被拦。
6. **双轨可控**：迁移期旧脚本为 CLI 的薄转发器，CI 翻转与旧轨删除解耦；`scripts/` 仅在最后一批真正清零。
7. **裁判统一**：CI 16 job 与 pre-commit 4 hook 全部切到 `coder`；全仓无 `node scripts/` 残留。

## 三、非目标

- 不动 `services/` / `libs/` / `sdk/` 的 Rust / Java 代码与架构。
- 不改发布产物形态：镜像清单、airgap tar 结构、deploy 骨架包语义均不变。
- 不做 MCP server（沿用前序 spec 裁决；CLI 的 `--json` 面为将来 MCP 化留接缝）。
- 不引入运行时依赖：CLI 保持零依赖 CJS、Node ≥ 18、白名单仅 `js-yaml`。JSONPath 判据用自研子集，不引入表达式引擎。
- 不改 CI job 划分、触发条件、GitLab CI Variables、部署机 swap-staging 流程——只换每个 job 的调用载体。

## 四、反目标（出现即失败）

- **旧轨删除早于 CI 切换**：任何一批里 `scripts/*.js` 被删除而对应 CI job 未翻转 → 立即失败。
- **守卫语义被弱化**：收编过程中任一门禁的检查项减少、阈值放宽、或新增可绕过的 skip 开关 → 立即失败。
- **命令名不收敛**：收编后命令面膨胀到 > 80 条，或同一能力出现两个命令名 → 失败。
- **skill 剧本仍有未标注的散文判据**：任何「看输出判断是否通过」而没有 JSON 判据的步骤 → 失败。
- **CI 与 CLI 判据不一致**：切换后 CI 结论与 `coder check` 结论不符 → 失败。
- **版本账本仍需人肉对账**：`coder release status` 必须一条命令给出 git tag / Makefile / CHANGELOG 三处一致性判定。

## 五、约束

- 平台：开发主机 Windows（cargo 编译/测试经 WSL2，`scripts/lib/host-runner.js` 的发行版选择逻辑收进 CLI）；CI 为 Linux 容器。
- 仓库硬规则：spec 先行、一任务一 commit 禁 amend/force-push、中文文档英文 commit、contracts 生成物禁手改、OpenAPI 由 utoipa 生成禁手编。
- `contracts/error-codes.yaml` 是**业务 API 错误码**，不混用工具层码；工具层另立 `contracts/tool-codes.yaml`。
- 守卫输入源 = 仓库工作区当前状态（非 git 历史）。脏工作区下 check 结果含未提交改动——这是正确语义，但必须在 skill 剧本里显式写明，避免 agent 误判。

## 六、已裁决事项（2026-11-02 用户拍板）

| # | 裁决 | 内容 |
|---|---|---|
| 1 | 改造边界 | **全量收编进 CLI**（不选薄壳包裹，不选只改 skill 指向） |
| 2 | 前端 skill | **删除** `prism-design` / `scaffold` / `update-shell` / `gen-page`，白名单收缩为 6 个 coder-* 脚手架（沿用 2026-10-30 裁决 #5） |
| 3 | 工具错误码 | 另立 `contracts/tool-codes.yaml`，由 `contracts-gen.js` 生成到 `packages/cli/lib/codes.js`（不进 Rust 侧 `digua_common-error`） |
| 4 | 守卫粒度 | **一个守卫一条命令**（`coder check spec` 只跑 spec 一个；`coder check all` 跑全部 14 个） |
| 5 | `.trae/skills` | 5 个独有 skill（`base-images-sync` / `docker-pull` / `e2e-test` / `lib-upgrade` / `service-audit`）**全部并入 `.agents/skills`**（功能已有对应命令，仅需剧本化）；`plan-eng-review` 两边重复，去重保留一份 |

## 七、架构：四层

```
编排层  22 skill 剧本 ────── 命令 → exit code → JSON 判据 → 失败处置
   │                              requires: [cmd, cmd]（CLI 反向校验）
   ▼
决策层  packages/cli 注册层 ── 声明式 spec（summary + flags + positional + run）
   │                              --help / 参数校验 / schema 全部由 spec 自动生成
   ▼
契约层  output.js + errors.js + contract.js
   │                              stdout 单行 JSON 信封 · stderr 日志 · 退出码 0/1/2/3/4
   ▼
实现层  lib/ 纯函数 + commands/ 带 IO 副作用 + lib/host-runner|services|shared-db|ui|prompt
```

### 7.1 契约层

- **信封**：`{ command, ok, exitCode, data, warnings[], errors[] }`，stdout 恰好一行，JSON 模式下所有日志与子进程输出转 stderr（沿用 `output.js` 现有实现，语义正确，保留）。
- **退出码**：`0` 成功 / `1` 未分类失败 / `2` 用法错误 / `3` 守卫未通过 / `4` 外部命令失败（沿用 `errors.js` 的 `EXIT` 表）。
- **未知 flag**：立即抛 `UsageError` → exit 2。**B0 必修**（现状静默通过）。
- **`--help` 任意位置生效**：`splitGlobals` 当前只在命令名前扫描 `--help`，命令后落入 `positional`。改为：命令 spec 声明 `flags` 后，spec 校验阶段统一识别 `--help` 并打印该命令的帮助（含 flags 表、示例、退出码含义），不执行 `run`。**B0 必修**。
- **守卫失败阈值**：每个守卫 spec 显式声明 `fatalOn: ['error']`（或等价），由契约层统一计算退出码，守卫实现不自己决定 exit。`check-spec` 现行「warn 不影响退出码」语义保持不变。

### 7.2 决策层：命令树

| 桶 | 条数 | 命令 |
|---|---|---|
| `check` | 14 | `service-standard` `spec` `contracts` `db-naming` `migration-state` `tenant-scope` `audit-routes` `doc-links` `java-sdk-drift` `openapi-drift` `compose-images` `dev-env` `todo-reconcile` `all` |
| `test` | 9 | `unit` `integration` `cargo` `host` `docker` `e2e` `bench` `perf-diff` `all` |
| `dev` | 8 | `up` `down` `ps` `logs` `restart` `setup` `demo-data` `ci-local` |
| `build` | 5 | `images` `cargo` `java-sdk` `base-images` `prune` |
| `release` | 10 | `status` `bump` `tag` `changelog` `manifest` `publish maven` `publish rust` `publish npm` `compose-template` `bundle` |
| `init` | 2 | `wizard` `dev-bootstrap` |
| `ops` | 10 | `secrets` `up` `down` `status` `logs` `verify` `upgrade` `reset-data` `seed-demo` `load-images` |
| 顶层 | 4 | `doctor` `schema` `version` `assets download` |

**命令膨胀的对冲**：`check` 14 条 + 总计约 62 条，对 agent 是可枚举负担。对冲机制两条——① `coder schema --json` 一次性输出全量契约（命令名、flags 规格、结果 schema、退出码）供 agent 自行检索；② `coder <cmd> --help` 局部查询。**命令名一经发布即冻结**，新增能力优先用子参数而非新命令。

### 7.3 实现层

- 纯逻辑（错误码比对、版本对账、命名规则、路径推导）下沉 `packages/cli/lib/`，不碰 IO，可在 `node --test` 直接测。
- 带 IO / 副作用的进 `packages/cli/commands/<bucket>.js`。
- `scripts/lib/` 五个工具模块（`host-runner` / `services` / `shared-db` / `ui` / `prompt`）被 CLI 独占迁入 `packages/cli/lib/`，依赖改为 `chalk` / `ora` / `prompts` → 无依赖 ANSI 实现 + readline 实现（前序 spec 已定此路径）。
- `scripts/lib/prompt.js` 的交互在 `init wizard` 下保留人工模式；agent 模式走 flags + `--non-interactive`（`setup-wizard.js` 已支持该 flag，迁入时提升为 CLI 一等公民）。

### 7.4 数据流

```
argv
 → splitGlobals(--json / --cwd / --help / --version)
 → matchCommand（最长匹配）
 → spec 校验（未知 flag → UsageError exit 2；--help → 打印帮助 exit 0）
 → run(args, ctx{root, context, out, json, cwd})
 → Result { data, human, exitCode?, warnings?, errors? }
 → output.emit() → stdout 单行信封 + stderr 日志
```

**三种 data schema**：

| schema | 用于 | 关键字段 |
|---|---|---|
| `check` | 14 个守卫 | `findings[]`（`severity: error\|warn`、`code`、`target`、`message`）、`summary: {error, warn, checked}` |
| `action` | build / release / publish / init | `action`、`changed[]`、`artifacts[]`、`commit` / `tag` |
| `status` | doctor / release status / ops status / test | `state[]`（`name`、`status: ok\|degraded\|fail`、`detail`） |

### 7.5 编排层：skill 剧本格式

每个步骤四元组：**命令 → 期望退出码 → JSON 字段判据 → 失败处置**。散文只保留在「前置」与「异常处置」两节。

```yaml
- id: regenerate-openapi
  command: coder build openapi
  expect_exit: 0
  assert:
    - path: $.data.services_failed
      op: "=="
      value: 0
  on_fail:
    - when: $.data.services_failed
      action: retry-with-CARGO_TARGET_DIR
    - when: "$"
      action: abort
```

- **判据语言**：JSONPath 子集（`$.a.b`、`$.a[0].c`、`$.a[*].d`）+ 比较器（`==` / `!=` / `>` / `contains`）。自研，不引入表达式引擎。
- **判据自校验**：CLI 在剧本加载时验证每条 `path` 可解析、比较器合法；剧本自身写错在 CI 被拦，而非等 agent 跑到一半。
- **`requires` 声明**：剧本头部列 `requires: [build.openapi, check.spec]`；CLI 反向校验被引用的命令是否存在——**命令改名导致的剧本漂移在 CI 阶段暴露**。
- **仓库脏状态声明**：涉及工作区检查的步骤，剧本显式写明「检查当前工作区状态，含未提交改动」，避免 agent 误判为历史回归。

## 八、迁移批次

原则：**每批一个可独立回滚的 commit 序列，批末 CI 必须绿**；不做一次性全量切换。

| 批 | 内容 | 理由 | CI 切换点 |
|---|---|---|---|
| **B0 契约** | 修 `--help` / 未知 flag；引入声明式 spec；`coder schema`；`tool-codes.yaml` + 生成器 | 后续每批复用此契约 | 无（仅 CLI 自身测试） |
| **B1 check** | 14 个守卫收进 `coder check` | 守卫是 AI 自证主力，无副作用，最适合先机读化 | CI 6 job + pre-commit 4 hook → `coder check` |
| **B2 release** | 补 `release bundle` / `manifest` / `changelog`；发布线补齐 | 发布高危，契约须严 | CI publish 3 job |
| **B3 test** | `test host/docker/cargo/e2e/bench/perf-diff` | 耗时，AI 需结构化结果 | CI rust_ci / e2e / k6 |
| **B4 dev** | `dev up/down/ps/logs/ci-local/demo-data` | 开发循环高频，AI 调用最频繁 | CI 局部 |
| **B5 build** | `build images/cargo/java-sdk/base-images` | 镜像构建慢，AI 需进度与结果 | CI 镜像构建 |
| **B6 init** | `init wizard`（交互转 flags）/ `dev-bootstrap` | 唯一交互阻断点，需专门非交互设计 | — |
| **B7 ops** | `dist-template/bin` 11 个收进 `coder ops` | 下游部署机场景，独立上下文 | 部署机 CI |
| **B8 剧本化** | 22 skill 重写为四元组；`.trae/skills` 归并；删 4 个前端 skill；白名单收缩为 6 | 必须等命令面齐了才能写判据 | — |
| **B9 清零** | 删 `scripts/`；根 npm script 89 → ~12；清 `dist-template/release/` 254 个陈旧 JS | 收尾 | 全仓 grep 无残留 |

### 8.1 双轨收敛机制

每批迁移后，**旧脚本变成 CLI 的薄转发器**而非立即删除：

```js
// scripts/check-spec.js（迁移后）
require('../packages/cli/bin/coder.js');  // 内部转调 coder check spec，透传退出码
```

这样人、CI、skill 任何一方都还有旧路径可用，**CI 翻转与旧轨删除解耦**。B9 才真正删文件。旧脚本内不得残留任何业务逻辑，只允许转发。

## 九、错误处理

| 层 | 处理 |
|---|---|
| 用法错误（未知命令 / 未知 flag / 缺参） | `UsageError` → exit 2，`errors[0].code = 'USAGE'` |
| 守卫未通过 | `GuardError` → exit 3，`data.findings[]` 给出每条 error 级 finding 的 `code` / `target` / `message` |
| 外部命令失败（cargo / docker / mvn 非零） | `ExternalError` → exit 4，`details` 带命令与 stderr 摘要 |
| 交互阻断（需人工输入但无非交互路径） | `UsageError` + 明确的 `--non-interactive` 提示 → exit 2，**不得挂起等输入** |
| 守卫内多 finding | 单次执行聚合全部 findings，不在首个失败即中断（`check-all` 需跑完全部子守卫再汇总） |

**工具错误码**：`contracts/tool-codes.yaml` 定义工具层码（如 `GUARD_FAILED` / `SPEC_MISSING_TITLE` / `VERSION_DRIFT` / `UNKNOWN_FLAG`），由 `contracts-gen.js` 生成 `packages/cli/lib/codes.js`。与业务 `error-codes.yaml` 物理隔离，不进 Rust 侧 `digua_common-error`。

**生成物纪律**：`packages/cli/lib/codes.js` 为生成文件，**禁手改**，与 `libs/digua_common-error/src/codes.rs` 同等对待。

## 十、测试策略

每条命令至少三条测试：

1. `--json` 输出可被 `JSON.parse` 解析，且 stdout 恰好一行。
2. 缺参 / 未知 flag → exit 2。
3. 业务失败场景 → exit 3 或 4（按 §九）。

守卫类额外测：**error 级 finding → exit 3；仅 warn 级 → exit 0**。

分层沿用现有先例（`packages/cli/test/` 14 个文件）：
- **纯函数测试**：`lib/` 内错误码比对、版本对账、命名规则、路径推导。
- **命令层测试**：信封形状、退出码、`--help` 任意位置生效、未知 flag 拒绝。

**契约回归测试**（新增，防退化）：
- `coder schema --json` 输出的命令清单与 `COMMANDS` 注册表一致。
- 每个 skill 剧本的 `requires` 引用的命令均存在。
- 每个 skill 剧本的 `path` 表达式可解析。

## 十一、留给建造者的裁量

- 各桶内具体命令的参数（flag 名、默认值、是否支持 `--dry-run`）。
- `test` 桶的 `unit` / `integration` 与现有 `run-tests-host` / `run-tests-docker` 的对应关系与切分粒度。
- `dev ci-local` 是否保留 `ci-local-run.js` 的分组语义（当前分 fast / slow / docker 组）。
- `check all` 的执行顺序与并发策略（是否并行、是否可 `--only` 过滤）。
- skill 剧本 YAML 前置元数据的完整字段集。
- `assets download` 是否也并入 `build` 桶（当前置于顶层）。
- `dist-template/release/` 下 254 个陈旧 JS 的清理方式（一次性删 vs 分批删）。
- `packages/cli` 发布到 npm 后，下游业务仓安装 `@digua/cli` 的版本对齐策略。

## 十二、验收判据

1. `coder --help` 列出全部七个桶的命令；`coder schema --json` 输出全量契约且与注册表一致。
2. `coder <cmd> --help` 在**任意**位置生效且不执行副作用；未知 flag 一律 exit 2。
3. `coder check all --json` 输出 `findings[]` 与 `summary`，error 级 → exit 3、仅 warn → exit 0。
4. `scripts/` 目录清零（仅剩 CLI 不可收编的异类，如 k6 场景）；全仓 `grep 'node scripts/'` 在 CI / docs-site / skills 无残留。
5. CI 16 job 与 pre-commit 4 hook 全部调 `coder`，门禁语义与迁移前等价（逐条比对检查项数量与阈值）。
6. 22 个 skill 剧本全部为四元组格式，每条 `assert` 有可解析的 `path`，`requires` 引用的命令均存在；4 个前端 skill 已删除，白名单为 6 个 coder-*。
7. 发版全流程可由 agent 按 skill 剧本执行，每步以 JSON 输出自证，无需人肉补位。
8. 现有测试全绿（`test:all` 语义等价），新增契约回归测试覆盖命令清单一致性与剧本可解析性。
9. 下游分发包：`make secrets/up/verify/upgrade` 行为不变（内部走 `coder ops`）；airgap 包含 CLI 本体、离线可用。
