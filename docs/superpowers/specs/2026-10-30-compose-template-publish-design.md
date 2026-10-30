# docker-compose 基础模板发布命令设计

- 日期：2026-10-30
- 状态：approved（用户 2026-10-30 明确下单：lmms 提供 pnpm 发布命令，下游 ai-pm 复制模板引用）
- 范围：lmms packages/cli 新增 `coder release compose-template` + 根脚本 `compose:template`

## 意图

ai-pm 等下游仓的运行时对接走「模板复制」通道：lmms 把自己的 `infrastructure/`
编排布局发布为**版本化基础模板**，下游复制到本仓后增改自己的服务定义，
经同名 compose 项目（目录名 `infrastructure`）自动并入 `infrastructure_superpowers` 网络。

## 命令

`coder release compose-template [-v <version>] [-o <outdir>]`

- 来源：`<root>/infrastructure/`
- 产物：`<root>/release/compose-template/coder-compose-template-<version>/`（未打包目录，便于直接复制）
  + `TEMPLATE_INFO.json`（version / generatedAt / fileCount）
- 版本缺省取最新 `v*` git tag（无 tag 时必须 `-v`）
- 根脚本：`pnpm run compose:template`

## 包含 / 排除

- 包含：`docker-compose*.yml`、`base-images.json`、`.env.prod.example`、`README.md`、
  `config/**`、`docker/**`、`mysql-init/**`、`mysql-replica/**`、`nginx/**`、`scripts/**`
- 排除：`.env`（真实密钥）、`vendor/`（构建期大件）、`.grok/`、`AGENTS.md`、`STATE.md`、
  `LOOP.md`、`loop-*.md`（仓内会话文件）；规则 = 跳过除 `.env.prod.example` 外的一切点开头文件/目录
  + 跳过上述显式名单

## 非目标

- 不做 tar.gz 打包（airgap 离线包已有发布链路）；不做 npm 包化（复制即用）；
- 不改模板内容本身（原样复制，适配由下游 README 指引完成）。

## 验收

1. 单测覆盖收集/排除规则与产物清单；`pnpm run test:cli` 全绿。
2. 实跑 `pnpm run compose:template` 产出含 TEMPLATE_INFO.json 的模板目录，排除项不出现。
