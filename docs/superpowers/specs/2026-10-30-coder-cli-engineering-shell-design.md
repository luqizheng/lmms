# 工程化外壳收敛设计：@digua/cli（bin: coder）

- 日期：2026-10-30
- 状态：draft（brainstorming 产出，用户裁决已并入，待批准）
- 基线分支：`feat/token-client-credentials`（含去前端化改造 c846b5ee）

## 一、意图与背景

框架本体的工程化外壳——发布、初始化、开发流程、AI 协作——目前由 **122 个脚本文件 / 19,239 行 JS**、根 package.json **86 条 npm script**、**24 个 CI job** 驱动，全部散装：

| 桶 | 文件数 | 行数 | 代表 |
|---|---|---|---|
| A 发布/构建/打包/codegen | 50 | 7,936 | build-docker、publish-libs、release.js、release-deploy.js |
| B 初始化/upgrade | 7 | 1,461 | setup-wizard(1,016行)、dev-setup、generate-secrets、upgrade.js |
| C 开发/测试 | 40 | 6,233 | run-tests-docker/host、build-cargo、ci-local-run、demo-data、k6×9 |
| D 守卫/检查 | 16 | 2,470 | check-service-standard、check-spec、contracts-gen --check、check-db-naming |
| E 共享工具 | 9 | 1,139 | scripts/lib/{ui,prompt,utils,host-runner,services,...} |

散乱的直接证据：版本号读写散在 4 处（release-cli lib/version+makefile、分发包 bin/upgrade、check-packages-publish）；容器内跑测试 3 处各写一套 volume 映射；UI/prompt 封装 2 套；demo 数据 2 套实现；migration state 检查 2 个；`release/dist-template/release/` 快照目录还躺着 254 个陈旧 JS 拷贝。Rust 侧没有 xtask，编译/测试/发布全靠 JS 调度。

同时「AI 化」目标定调为 **AI 主导执行**：agent 说意图、编排命令、自校验结果。但现状 22 个 skill 全是纯文档（3 个仅带 preflight 脚本），没有 MCP server，AI 只能靠读文档自己拼命令，输出无人机读格式。

2026-10-30 用户裁决链：**cms-release 放弃**（能力并入新外壳）；外壳用 **Node 收敛为单一 CLI 包**（不重写 Rust）；**连带分发包一起改**；**四个前端 skill 全部下架**。

## 二、目标

1. **单一 CLI 包**：`packages/cli`（npm 包名 `@digua/cli`，bin 名 `coder`），零依赖 CJS，Node ≥ 18，pnpm workspace 成员（与 @digua/contracts、@digua/http 同列）。
2. **命令面收敛**：子命令按桶组织——`coder build`（cargo/docker/镜像）、`coder test`（host/docker/e2e/bench）、`coder dev`（infra/up/down/logs/demo-data/ci-local）、`coder check`（全部守卫 + contracts 生成）、`coder release`（publish 四路、打包、**版本账本对账与 bump**）、`coder init`（setup-wizard/dev-setup/generate-secrets）、`coder ops`（面向分发包部署机的 load-images/start/verify/upgrade/status/logs/stop/reset）。完整命令树由建造者设计；coder 探测运行上下文并按上下文暴露子集（见裁决 #8），下游业务仓（无根 Cargo.toml、含 Java 服务、apps/ 布局）下的命令适配是建造者任务之一。
3. **版本账本收口**：git tag（带 v）/ dist-template Makefile IMAGE_TAG / CHANGELOG 三处对账 + bump 进 `coder release status|bump`，接管已放弃的 cms-release 的对账能力。
4. **AI 主导的执行面**：所有命令支持 `--json`（稳定 schema）+ 稳定退出码；`coder status`/`coder verify` 类命令可机读，agent 每步执行后自证成功；人读输出是默认态、机读是增强态。
5. **分发包连带改**：dist-template 的 `bin/*.js` 收进 `coder ops`（分发包内附 CLI 文件本体，零依赖可直接 `node` 调）；Makefile 保留为下游入口、target 内部转发 coder；airgap 包定位不变（新环境首次初始化）；下游升级模型不变（镜像 tag + kellnr/Maven/npm 包版本）。
6. **入口收敛**：根 npm script 86 → 约 10 条薄封装；CI 24 个 job 改调 `coder ...`；tag 四路 publish 的 publish_packages 路新增 @digua/cli（与 contracts/http 同机制：按 tag 版本 npm version 后发布，check 前置）。
7. **skills 升级为编排剧本**：每个 skill 的步骤从「描述性文字」改为「确定命令 + 预期 JSON 判据」；`DOWNSTREAM_SKILL_WHITELIST` 收缩为 6 个 coder-* 脚手架（update-shell/scaffold/gen-page/prism-design 四个前端 skill 删除）；`.agents/skills/` 与 `.trae/skills/` 两套 skill 目录收敛为一套（保留 `.agents/skills/`，`.trae` 侧迁移或别名，建造者定）。
8. **删除清单**：`release/release-cli/`（cms-release）；`release/dist-template/release/` 下 4 个快照目录约 254 个 JS；scripts/ 被收编的旧脚本；各重复实现归一（容器执行、UI、版本、demo 数据、migration 检查各留一处）。

## 三、非目标

- 不动 `services/`、`libs/`、`sdk/` 的 Rust/Java 代码与架构。
- 不改发布产物形态：镜像清单（8 服务 + mysql-replica）、airgap tar 结构、deploy 骨架包语义均不变。
- 不用 Rust 重写工具链（用户裁决保留 Node 外壳）；k6 压测场景留在 `tests/load/*.js`（k6 生态绑 JS），仅入口被 `coder test` 调度。
- 不改 CI 编排结构与门禁语义：job 划分、触发条件、GitLab CI Variables、部署机 swap-staging 流程不变，只换每个 job 的调用载体。
- 不做 MCP server（本轮不做；CLI 的 --json 面为将来 MCP 化留好接缝）。

## 四、反目标（出现即失败）

- 下游 Makefile 常用 target（`make up/verify/upgrade` 等）行为破坏。
- 守卫收编过程中任何一个门禁语义被弱化或可绕过。
- 发版完成后版本账本仍需人肉对账——`coder release status` 必须一条命令给出三处版本的一致性判定。
- 新旧两套入口长期并存：迁移完成后 `scripts/*.js` 必须清零（lib 并入 CLI），docs-site/skills/CI 中不得残留 `node scripts/` 调用。

## 五、约束

- 平台：开发主机 Windows（cargo 编译/测试经 WSL2，现有 `lib/host-runner.js` 的发行版选择逻辑收进 CLI）；CI 为 Linux 容器。
- 零依赖 CJS、Node ≥ 18，**白名单例外仅 js-yaml**（纯 JS、无原生绑定）：check-spec 与 openapi-codegen 是禁弱化门禁、输入为 utoipa/contracts 固定格式 YAML，自研解析器做逐字节等价验证的风险不可接受；js-yaml 随分发包/airgap 一并附带（连同其 node_modules 目录拷入），不影响离线使用。其余外部依赖不进白名单：chalk/ora 换无依赖 ANSI 实现（release-cli 的 lib/ui.js 是现成先例）；prompts（×3）换 readline 无依赖实现；mysql2 随 demo-data 收敛为走网关路径自然消除。
- 内网 registry：npm.kudi.repos；pnpm workspace 成员；不进分发包的 node_modules（零依赖直接拷文件）。
- 仓库硬规则：spec 先行、一任务一 commit 禁 amend/force-push、中文文档英文 commit、contracts 生成物禁手改、OpenAPI 由 utoipa 生成禁手编。
- 迁移期间 CI 必须保持绿：允许分阶段切换（每桶迁完翻转对应 job 的调用），不允许全量切换一次赌。

## 六、已裁决事项（全部 2026-10-30 用户拍板）

| # | 裁决 | 内容 |
|---|---|---|
| 1 | cms-release 放弃 | release/release-cli 不作发布入口，随重构删除；版本对账能力并入新 CLI |
| 2 | AI 化形态 | AI 主导执行：agent 编排命令并自校验；命令与输出为 agent 设计（--json、稳定退出码） |
| 3 | 外壳语言 | Node 收敛为单一 CLI 包（零依赖 CJS），不重写 Rust |
| 4 | 范围 | 连带分发包（dist-template bin/*.js、Makefile）一起改 |
| 5 | 前端 skill | 四个全部下架删除；白名单收缩为 6 个 coder-* 脚手架 |
| 6 | CLI 命名 | bin = `coder`，npm 包 = `@digua/cli` |
| 7 | 基线 | 基于 feat/token-client-credentials；合并 main 时丢弃 release-cli/dist-tools 残留 |
| 8 | 运行上下文 | **三种上下文**：上游 monorepo / dist 部署机 / 下游业务仓；coder 探测环境、各暴露有意义子集；下游仓经 pnpm 安装 @digua/cli，6 个脚手架 skill 剧本在下游直接可用 |

## 六·补、建造者检查后补充的裁量（我的裁定，待用户复核）

1. **依赖白名单 = 仅 js-yaml**（理由见「五、约束」）。prompts/mysql2/chalk 不进白名单，替代路径如上。
2. **发布安全边界**：
   - `coder` **永不自动 push tag**。`coder release bump` 只改 Makefile + CHANGELOG 并 commit；打 tag（`coder release tag vX.Y.Z`）与推送（`git push origin main vX.Y.Z`）是显式独立步骤。
   - 版本权威源 = **git tag**（与 CI 的 `CI_COMMIT_TAG` 派生一致）。三处不一致时 `status` 如实报告、`bump` 默认拒绝执行，`--force --from-tag` 才允许以 tag 为源收口其余两处。
   - CHANGELOG 条目由 agent 按剧本从 commit log 起草，经 `bump --changelog <file>` 传入，不接受命令自动生成正文。
   - skill 剧本中推 tag 是显式确认点：agent 执行前向人确认（误推 tag = 触发四路发布，属不可逆外向动作）。

## 七、留给建造者的决策

- 完整命令树、子命令参数与 --json schema 形状；退出码表。
- 16 个 check-* 守卫的合并/保留粒度（哪些并成一条命令的子模式、哪些保持独立）。
- 迁移分批顺序与每批的 CI 切换点。建议顺序 = 用户点名顺序：发布+版本收口 → 初始化+dist-template → 开发测试 → 守卫+skills 剧本化；每批一个 commit 序列、可独立回滚。
- lib 结构：scripts/lib（host-runner/services/shared-db/prompt…）与 release-cli lib（version/makefile/repo）的合并归置。
- demo-data 双实现（直连 1,052 行 vs 走网关 31 行）保留哪条路径为主。
- setup-wizard 的交互模式保留方案（人用交互向导、agent 用非交互 flags + --json）。
- 每个 `coder` 命令的测试策略（先例：release-cli 17 个纯函数 node --test；CLI 命令层测试 + 纯函数测试分层）。
- skill 剧本化的具体格式（步骤 → 命令 → JSON 判据的模板）。
- `.trae/skills/` 7 个平台 skill 的归并方式。
- 上下文探测机制（各上下文的标记文件/目录特征）与「上下文 × 命令子集」矩阵；下游仓下 openapi 导出、迁移状态检查等命令的适配方案。

## 八、验收判据

1. `pnpm run` 只剩约 10 条薄封装；`scripts/` 目录清零或仅剩 CLI 不可收编的异类（k6 场景等）；全仓 grep `node scripts/` 在 CI/docs-site/skills 无残留。
2. `coder release status --json` 一条命令输出 git tag / Makefile / CHANGELOG 三处版本对账结果。
3. 发版全流程可由 agent 按 skill 剧本执行，每步以 JSON 输出自证，无需人肉补位。
4. 下游分发包：`make secrets/up/verify/upgrade` 行为不变（内部走 coder）；airgap 包含 CLI 本体、离线可用。
5. 现有测试全绿（test:all 语义等价），新增 CLI 命令层测试覆盖每个子命令。
6. 下游业务仓内（pnpm 安装 @digua/cli）：6 个脚手架 skill 剧本所引用的 coder 命令子集可端到端走通；命令在上游 monorepo、dist 部署机、下游仓三种上下文下的可用子集与文档一致。
