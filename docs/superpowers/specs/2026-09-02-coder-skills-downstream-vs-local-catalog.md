# Coder Skills 下游(A) / 仓库(B) 盘点与设计 —— v3（锁定白名单与发布方式）

- 日期：2026-09-02（v3，锁定 owner 决策：白名单 6 项 + release.js 白名单生成）
- 类型：设计（实施蓝图，代码变更待 dispatch）
- 状态：**决策已锁定，实施未开始**
- 关键结论：发布产物的 `.agents/skills/` 只装 **6 个下游 scaffold skill**（Java 3 + Rust 3），由 `release.js` 从仓库单一真相源 `.agents/skills` **白名单选择拷贝**；SDK 发布 / repo-local skill 一概不下沉。

## 0. 相关背景摘要

讨论中确认过的事实（含 release 链路证据）：

- 发布入口 `pnpm run release:build` → `release/dist-template/scripts/release.js` → 输出骨架目录 `release/dist-template/release/coder-micro-services-{VERSION}/` 与同名 `-airgap.tar.gz`。
- 发布模式是「平台核心服务以镜像发行 + 开发骨架」，下游在**空的 `services/`** 里用 skill 自助创建业务微服务。
- **现状漂移**：`dist-template/README` 声称下游技能为 `skills/` 下 6 个 AI skill（3 Rust + 3 Java），但 `release.js`（~L251）实际是 `cp(仓库/.agents/skills → DIST/.agents/skills)` **整目录全量拷贝**，把 SDK 发布/CI 诊断/性能剖析等 repo-local skill 也一并封包。
- 下游同步：`.agents/**` 属默认 `.upstream-sync.yaml` 的 three-way-merge 范围，多拷贝即多噪音。
- owner 澄清：下游不需要本项目的 SDK 相关发布技能；下游主要会创建业务微服务。

## 1. 最终决策（owner 确认）

### D1 下游白名单 = 6 项 scaffold（既有性/新增都进 dist）
从仓库 `.agents/skills` 中，以下 **6** 个是「下游服务创建与维护要用」的白名单：

| # | skill（目录名在 `.agents/skills/`） | 语言 | 作用 | dist 是否下沉 |
|---|---|---|---|---|
| 1 | coder-new-service | Rust | 新建 Rust 业务微服务骨架（8-item） | ✅ |
| 2 | coder-add-route | Rust | Rust 服务加路由 / OpenAPI | ✅ |
| 3 | coder-migrate-db | Rust | Rust 双言论迁移 | ✅ |
| 4 | coder-new-java-service | Java | 新建 Java(Spring Boot3+JDK21)业务微服务骨架 + docker-compose/网关 | ✅ |
| 5 | coder-add-java-route | Java | Java 服务加 Controller/DTO | ✅ |
| 6 | coder-migrate-java-db | Java | Java Flyway 迁移 | ✅ |

其余全部为 B（repo-local，不外发），含：`coder-sdk-publish-{npm,rust,maven}`、`coder-service-release`、`coder-new-?(non whitelist)`、`coder-ci-diagnose`、`coder-contract-trace`、`coder-perf-profile`、`coder-rust-unit-test`（不入白名单）、`coder-migration-rollback`（不入白名单）、`coder-openapi-contracts-sync`、`coder-microservice-review-checklist`、`karpathy-guidelines`、`plan-eng-review`，以及平台级 `.trae/skills/*`。

> 注：`coder-rust-unit-test` / `coder-migration-rollback` 与 Java/Rust scaffold 工作流相邻，但按 D1 仅这 6 项进包；如 owner 希望也随 Rust 业务单位一并下发，可后续在白名单追加而不动架构。

### D2 发布方式 = release.js 白名单生成 / 单一真相源
- **不要在 release/** 里维护一份 scaffold SKILL.md 副本**（会与本仓 `.agents/skills/` 双源漂移）。
- 让 `release.js` 在执行骨架同步时分两种来源：
  1. `.agents/skills/<白名单 6 项>` → **选择性拷入** `DIST_DIR/.agents/skills/`（保留 6 项 + 子脚本）。
  2. 其余 `repo-local` skill → **不拷贝**；可选地在 dist 写一份 `DIST_DIR/.agents/README.downstream.md`，说明"其余 repo-local skill 属框架仓，不外发；如需自行创建业务服务见下方 6 个 scaffold"。
- 用以下常量做单一来源清单（与 D1 表同名）：
  ```
  RELEASE_DOWNSTREAM_SKILL_WHITELIST =
    coder-new-service, coder-add-route, coder-migrate-db,
    coder-new-java-service, coder-add-java-route, coder-migrate-java-db
  ```

## 2. 核心好处 vs 注意点

好处
- 下游 `services/` 空骨架下只能看到他要的 scaffold —— 不误跑 SDK/CI 发布技能。
- `cms-sync` three-way-merge 只对比 6 项，少冲突、少回写。
- 修改 skill 正文仍只在本仓 `.agents/skills` 一处编辑；release 时白名单拷出即最新。

注意点
- dist-template/README 目录树写的 `skills/` 与实现 `.agents/skills` 需统一（后续把 README 章节改对或用 alias 说明），避免给下游/自维护者误读。
- 白名单内的 scaffold（尤其 new-service/new-java-service）正文引用了本仓脚本（`run-tests-docker.js`、`check-service-standard.js`、`services/agents-template.md` 等）；这些对象**也必须随 dist 下发**（`scripts/`、`services/agents-template` 已在 release.js 的同步条目内）。审查确认它们都进不依赖第三方私有产物。
- 下游若 Dockerfile/网关模板有差异，scaffold 假设到网关/consul 结构，仍需遵守 dist 事实标准；不适配处应由下游通过 skill 内的插桩节自行 override。

## 3. 实施蓝图（待授权后执行，不包含在本 doc）

1. `release.js`：新增 `DOWNSTREAM_SKILLS` 白名单数组（6 项），把现有整目录 `.agents/skills` 拷贝改为按白名单循环 `cp`（保留内部子目录）。
   - 追加 `DIST_DIR/.agents/README.downstream.md`（若不存在 children artifact），明示不外发的 repo-local skill 清单。
2. `dist-template/README`（骨架内）目录树与快速开始对齐 `.agents/skills` 且只展示 6 个 scaffold；修正当前"skills/6 skills"表述。
3. （可选）`root AGENTS.md` Skill 目录表拆成两节：`下游(downstream, 进 release/.agents/skills)` = 6；`仓库(repo-local, 不外发)` = 其余；前端标注 —— 以便维护者长期保持白名单准确。
4. 校验：仓库层面 `node scripts/check-*` 与 `node scripts/dump-openapi-specs.js` 不受影响；对 release 运行 `release:test`；发布前执行 AGENTS 记载的「发布前对账 SOP」确认 dist 产物 .agents/skills 恰为 6 项。

### 验收判据（doD）
- [ ] dist/.agents/skills 顶层只含白名单 6 项 skill 子目录（新增别名/继承子组件不算）；
- [ ] 仓库 `.agents/skills/` 仍完整保留全员（发布副本不删除本仓库源）；
- [ ] 无 repo-local（SDK 发布/CI 诊断等）混入 dist/.agents/skills；
- [ ] release:test 通过，README 结构表述与实现一致。

## 4. 未决 / 后续
- `coder-rust-unit-test`、`coder-migration-rollback` 是否要与 Rust scaffold 同包 —— 默认排除，owner 可随时扩白名单。
- Java scaffold 的「发布 + 调试」两套 docker-compose 产出（§5 draft 见下方扩展物）会随 4/5 skill正文发布 —— 与本次锁定的 6-白名单正交，可在 skill 本身演进单独做，二者兼容。
