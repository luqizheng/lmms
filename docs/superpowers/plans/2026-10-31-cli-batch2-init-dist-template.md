# @digua/cli 初始化与分发包收敛 实施计划（批次二）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把初始化链（setup-wizard / dev-setup / check-dev-env / images-pull）收编为 `coder init` 命令组，把分发包运维面（dist-template `bin/*.js` 全部 10 个 + Makefile 转发）收编为 `coder ops` 命令组，airgap/deploy 包附 CLI 本体，修复侦察实证的五处既有缺陷（dist 上下文判定、verify 端口表、airgap manifest 缺失、Makefile 死 target、check-dev-env 三处过时）。

**Architecture:** 沿用批次一骨架：命令层（`commands/*.js`）只做参数解析与编排，业务逻辑放可注入 `exec`/`fs`/`crypto` 的 `lib/` 模块；ops 命令在 dist 上下文运行、不得依赖 `scripts/lib/*`（入包零依赖）；分发包内 CLI 固定在 `cli/` 目录（`cli/bin/coder.js`），Makefile 保留为下游入口、target 一律转发 coder。

**Tech Stack:** Node.js ≥18，CommonJS，`node:test` + `node:assert/strict`，无运行时依赖（js-yaml 属批次四，本批不引入；compose 文件不解析 YAML，`image:` 扫描保持逐行正则）。

**Spec:** `docs/superpowers/specs/2026-10-30-coder-cli-engineering-shell-design.md`（§二.2 命令面、§二.5 分发包连带改、§六裁决 #4/#8、§七批次顺序、§八验收判据）；格式与约束先例 = `docs/superpowers/plans/2026-10-30-coder-cli-release-consolidation.md`（批次一）。

## Global Constraints

- 批次一 Global Constraints 全部继承：`--json` stdout 单行信封、退出码表 `0/1/2/3/4`、零依赖 CJS、永不 push/永不自动建 tag、Windows(Git Bash)+Linux 双平台只用 node API 与 `execFileSync`、被收编脚本一律删除不做转发壳、中文文档英文 commit、一任务一 commit 禁 amend/force-push。
- **ops 命令零依赖入包**：`commands/ops.js` 与其 lib 不得 `require` `scripts/lib/*`、不得用 npm 包；分发包只拷 `packages/cli/{bin,lib,commands,package.json}` 到 `cli/`，无 node_modules 即可运行（批次一 `sdk_publish` job 已实证此模式，`.gitlab-ci.yml:600-604`）。
- **禁 shell 字符串拼接**：被收编脚本里的 `curl`/`sleep`/`openssl`/`chmod`/`ls -la`/`docker inspect` 全部换 node API（`node:http`/`node:crypto`/`node:fs`）或 `execFileSync` 参数数组；JSON 模式下子进程输出走 stderr（`lib/spawn.js` 先例）。
- **行为等价锚点**（反目标红线，设计稿 §四）：同一分发包上 `make secrets/up/down/status/logs/verify/reset-data/seed-demo/load-images/upgrade` 的对外语义迁移前后不变；`.gitlab-ci.yml` CODER_* 同步与五个配置文件（`.docker-registry.json`/`.cargo/config.toml`/`.cargo/credentials.toml`/`.mvn/settings.xml`+`maven.config`/`.npmrc`）的写入契约不变（消费方：`packages/cli/lib/maven-publish.js:39-54`、`scripts/base-images-sync.js:41-43`、CI `.gitlab-ci.yml:59-71`）。
- **版本语义**：镜像 tag 不带 `v`（`.env.example:8-11` 注释）；`ops upgrade` 接受带 `v` 前缀输入、剥前缀后写 `.env`。
- **批次边界**：`release/dist-template/scripts/*`（release.js / release-deploy.js / transforms）整体收编属批次三；本批只做 Task 8 列明的最小接线改动，接线之外的行不动。

## Review Focus

1. **CRLF/BOM 行为**：wizard 的 `.gitlab-ci.yml` 逐行重写与 `ops upgrade` 的 `.env` `IMAGE_TAG=` 改写必须保留原文件行尾（Windows checkout 可能 `\r\n`），不得整文件转 LF；新文件不得引入 BOM（旧 `scripts/check-dev-env.js` 带 BOM，迁移时重新起文件）。→ Task 9 迁移既有 `setup-wizard.test.js` 幂等/行尾用例 + 新增 CRLF fixture 用例；Task 6 的 `rewriteEnvLine` 用 `\r\n` fixture 断言字节级保留。
2. **非 TTY/CI 挂起**：`coder init wizard` 在 stdin 非 TTY 时必须自动走非交互路径（env→现有配置→默认值），不得挂起等输入；ops 命令一律无交互。→ Task 9 prompt 层单测 mock `isTTY=false` 断言零 readline 调用。
3. **幂等重跑**：`ops secrets` 重跑只替换仍为 `__CHANGE_ME__` 的行、JWT 密钥已存在则跳过；`ops upgrade` 同版本重跑 `.env` 不变；wizard CI 同步二次运行字节级不变。→ Task 2 双跑断言；Task 6 双跑断言；Task 9 迁移的既有单测。
4. **Windows/Unix 双平台**：`chmod 600` 在 Windows 失败不致命（try/catch 降级）；所有路径 `path.join`；compose/docker 调用参数数组。→ Task 2 注入 fakeFs 断言 chmod 容错；Task 1 compose 封装断言 args 数组形状。
5. **stdout 纯净与退出码**：`--json` 模式 stdout 只一行信封；`ops verify` 有端点失败退 `1`；`ops load-images` 在 manifest 缺失时降级 glob 而非退 `2`（修复点）。→ Task 4/5 按批次一 `capture()` 先例断言。

---

### Task 1: dist 上下文判定修复 + compose 执行封装

**Files:**
- Modify: `packages/cli/lib/context.js:19-25`
- Create: `packages/cli/lib/compose.js`
- Create: `packages/cli/lib/env-file.js`
- Modify: `packages/cli/commands/release.js:223`（doctor 的 dist 命令面）
- Test: `packages/cli/test/context.test.js`（扩展）、`packages/cli/test/compose.test.js`、`packages/cli/test/env-file.test.js`

**Interfaces:**
- Produces:
  - `detectContext(dir)`：dist 判定改为 `Makefile` + `infrastructure/docker-compose.yml` 同时存在（真实分发包 compose 在 `infrastructure/` 子目录，根目录无 compose——现存 1.6.0/7cde9007-deploy 产物实测；monorepo 判定不变且优先）。
  - `lib/compose.js`：`composeFile(root)` → `path.join(root,'infrastructure','docker-compose.yml')`；`composeArgs(root, args)` → `['compose','--env-file',path.join(root,'.env'),'-f',composeFile(root),...args]`；`runCompose({root, args, env, exec, cwd})` → execFileSync('docker', composeArgs, {env: {...process.env, ...env}, cwd: root})。ops 命令统一经此入口。
  - `lib/env-file.js`：`loadEnv(root)` → 按 `^(\w+)=(.*)$` 逐行解析 `.env` 为对象（不写 process.env；语义照搬 `bin/verify.js:14-19`）；`rewriteEnvLine(root, key, value)` → `^key=...$` m 正则幂等改写，无则文件尾追加，保留 EOL；`replacePlaceholder(root, key, value)` → 只替换 `^key=__CHANGE_ME__$`（语义照搬 `bin/generate-secrets.js:45-55`）。

- [ ] **Step 1: 失败测试**（三个文件）
  - context：fixture 含 `Makefile`+`infrastructure/docker-compose.yml` 无 `images/` → `dist`；含 `Cargo.toml`+`services/`+`libs/`+上述文件 → `monorepo` 优先；仅 `Cargo.toml`+`services/`（airgap 包形状，无 libs）→ `unknown`。
  - compose：`composeArgs` 返回数组且首二元素 `['compose','--env-file',<abs .env>,'-f',<abs yml>]`；`runCompose` 用 fakeExec 断言命令名 `docker`、args 数组、`env` 合并、`cwd=root`。
  - env-file：`\r\n` fixture 上 `rewriteEnvLine` 后重读值正确且行尾仍是 `\r\n`；key 不存在时追加；`replacePlaceholder` 对已替换值不动、对占位符替换。
- [ ] **Step 2: 跑测试确认失败**（模块/判定不存在）。
- [ ] **Step 3: 实现三模块 + doctor dist 命令面改为真名列表** `['ops secrets','ops up','ops down','ops status','ops logs','ops verify','ops reset-data','ops seed-demo','ops load-images','ops upgrade']`。同时把 `context.js:6` 注释「downstream rules land in batch 2」改为 batch 4——裁决：下游仓上下文探测不在本批（本批无下游侧消费命令；其命令子集 = 6 个脚手架 skill 剧本，属批次四，届时一并落探测规则与「上下文 × 命令子集」矩阵，设计稿 §七）。
- [ ] **Step 4: 验证**：`pnpm test:cli` 全绿。
- [ ] **Step 5: Commit** — `feat(cli): dist context detection fix and compose/env-file primitives`

### Task 2: `coder ops secrets`（generate-secrets 收编 + node:crypto 化）

> 裁决：设计稿 §二.2 把 generate-secrets 列在 `coder init` 名下，但 §二.5（dist-template `bin/*.js` 收进 `coder ops`）+ doctor 既有预告（`commands/release.js:223`「ops secrets … batch 2」）+ 其真实运行位置（部署机、`make secrets` 入口）三证一致指向 ops；init 名下不设 secrets 命令。

**Files:**
- Create: `packages/cli/lib/secrets.js`、`packages/cli/commands/ops.js`（本任务注册首个命令）
- Modify: `packages/cli/lib/commands.js`（合并 ops 导出）
- Test: `packages/cli/test/secrets.test.js`

**Interfaces:**
- Produces: `generateSecrets({ root, fs, crypto })` → `{ keypairGenerated: boolean, envCreated: boolean, replaced: string[], skipped: string[] }`。密钥生成 `crypto.generateKeyPairSync('rsa',{modulusLength:2048,privateKeyEncoding:{type:'pkcs8',format:'pem'},publicKeyEncoding:{type:'spki',format:'pem'}})` 写 `secrets/jwt_private.pem`/`jwt_public.pem`（已存在私钥则跳过且不动公钥）；`chmod 0o600` try/catch（Windows 失败仅 warning）；密码 `randomBytes(16).toString('hex')` ×4（MYSQL_ROOT_PASSWORD/MYSQL_PASSWORD/REDIS_PASSWORD/RABBITMQ_DEFAULT_PASS）、`randomBytes(32)` ×8（IAM_DEV_BT/TOKEN_DEV_BT/NOTIFICATION_DEV_BT/AUDIT_DEV_BT/PRUE_DEV_BT/GATEWAY_DEV_BT/CONFIG_DEV_BT/FILE_DEV_BT，清单逐字取自 `bin/generate-secrets.js:69-78`；原 TOKENS 映射 value 是死数据，不迁移）；`.env` 不存在时从 `.env.example` 复制；替换走 Task 1 `replacePlaceholder`。**不依赖 openssl**。
- 命令：`coder ops secrets [--json]`，人类模式结尾列 `secrets/` 目录清单（`fs.readdirSync`，替代 `ls -la`）。

- [ ] **Step 1: 失败测试**：注入 fakeFs+node:crypto——空 fixture：生成密钥对两文件+权限设置调用被记录；`.env.example`→`.env` 复制发生；12 个 key 全部进 `replaced`；**双跑**：第二次 `keypairGenerated=false`、手动把某行改成非占位符后该项进 `skipped`；chmod 抛错（fakeFs 抛 EPERM）不导致整体失败且 warnings 含一条。
- [ ] **Step 2: 确认失败 → 实现 → 通过**（`pnpm test:cli`）。
- [ ] **Step 3: Commit** — `feat(cli): ops secrets with node crypto, openssl-free and idempotent`

### Task 3: `coder ops up/down/status/logs/reset-data/seed-demo`

**Files:**
- Modify: `packages/cli/lib/compose.js`（加 `ps`/`logs` 便捷无必要则不加，命令层直接组 args）
- Create: `packages/cli/lib/health-wait.js`
- Modify: `packages/cli/commands/ops.js`
- Test: `packages/cli/test/health-wait.test.js`、`packages/cli/test/ops-lifecycle.test.js`

**Interfaces:**
- Produces:
  - `lib/health-wait.js`：`waitForContainerHealthy({ exec, container, maxSeconds=120, stepSeconds=2 })` → `Promise<{healthy:boolean,state:string}>`，轮询 `execFileSync('docker',['inspect','--format','{{.State.Health.Status}}',container])`（容器名前缀 `coder-micro-services-` 逐字保留，服务清单 `['mysql','consul','redis','rabbitmq','api-gateway']` 逐字取自 `bin/start.js:20`）；`probeUrl(url, {timeoutMs=3000})` → `Promise<{ok:boolean,status:number|null}>` 用 `node:http`（语义照搬 `scripts/check-dev-env.js:26-40` 的超时处理），供 Task 4 复用。
  - 命令（全部经 `runCompose`，语义照搬对应 `bin/*.js`）：`ops up` = `up -d` → 顺序 waitForContainerHealthy ×5 → 网关探活（URL 取 `.env` 的 `API_GATEWAY_PORT` 默认 3000，路径 `/health`——修正 `bin/start.js:50-52` 硬编码 3000 且与 verify 8080 互相矛盾的漂移，以 `.env.example:14` 为准）；`ops down` = `down`；`ops status` = `ps --format table`；`ops logs [svc]` = `logs -f --tail 100 [svc]`；`ops reset-data` = `down -v` 后内部调 `ops up` 流程；`ops seed-demo` = 读 `.env` 取 `API_GATEWAY_PORT`（原读 `GATEWAY` 变量，`.env.example` 无此键——修正为端口推导），`POST http://localhost:<port>/api/v1/seed`（node:http，语义照搬 `bin/seed-demo.js:25-27`）。
- [ ] **Step 1: 失败测试**：health-wait fakeExec 首两次返回 `starting` 第三次 `healthy` → healthy=true 且调用 3 次；`not-found` 立即返回；probeUrl 对立即响应 200 的 fake server ok=true（用 `node:test` 起 localhost socket 或注入 http 模块桩——按批次一注入先例选后者）。ops-lifecycle：`ops up` 依次产生 compose up → 5×inspect 序列（fakeExec 记录断言顺序）；`ops status/logs` 断言 args 形状；`ops down -v` 断言 reset-data 的序列。
- [ ] **Step 2: 确认失败 → 实现 → 通过**。
- [ ] **Step 3: Commit** — `feat(cli): ops lifecycle commands with node-native health wait and probes`

### Task 4: `coder ops verify`（端点表修正 + node http）

**Files:**
- Create: `packages/cli/lib/verify.js`
- Modify: `packages/cli/commands/ops.js`
- Test: `packages/cli/test/verify.test.js`

**Interfaces:**
- Consumes: Task 3 的 `probeUrl(url,{timeoutMs})`（verify 不再自造 http 探活）。
- Produces: `VERIFY_CHECKS(root, env)` → 检查表数组 `[{name,url}]`，成员与旧表相同（api-gateway/config-service/file-service/token-service/iam-service/audit-service/prometheus/grafana），**端口修正**为 compose 现实（`docker-compose-services.yml:108,134,164,201,264,385,472` 实测）：api-gateway `http://localhost:${API_GATEWAY_PORT|3000}/health`、config `:8085/health`、file `:8086/health`、token `:8082/health`、iam `:9081/health`、audit `:8084/health`、prometheus `:9090/-/healthy`、grafana `:3001/api/health`（旧表 `bin/verify.js:34-43` 的 8080-8085 逐项错位：8080 是已删除的 FRONTEND_PORT——`.env.example:15`）。`runVerify({root, env, probe, exec})` → `{checks:[{name,url,ok}], passed, failed, exitCode}`；结尾 `compose ps` 表格。
- [ ] **Step 1: 失败测试**：注入 probe 全 ok → exitCode 0；一项 fail → exitCode 1 且该项 url 出现在 result；表成员恰 8 项且 iam url 含 9081、gateway url 随 `API_GATEWAY_PORT=3100` 变为 3100。
- [ ] **Step 2: 确认失败 → 实现 → 通过**。
- [ ] **Step 3: Commit** — `feat(cli): ops verify with corrected port map and node-native probes`

### Task 5: `coder ops load-images`（manifest 缺失降级修复）

**Files:**
- Create: `packages/cli/lib/images.js`
- Modify: `packages/cli/commands/ops.js`
- Test: `packages/cli/test/images.test.js`

**Interfaces:**
- Produces（语义照搬 `bin/load-images.js`，行为仅一处刻意变更）：
  - `planLoads({ root, fs })` → `{ mode:'manifest', entries }` | `{ mode:'glob', files }`——`images/manifest.json` 存在则按 manifest 顺序列 `.tar`，**不存在则不再 exit 2**，改为 glob `images/*.tar` 全量装载（修复 airgap 包无 manifest 必挂缺陷：`release.js` 打包从不写 `images/manifest.json`，而旧脚本 `:45-48` 前置校验必炸；deploy 包不受影响）；两者皆无 → `GuardError`（exit 3）提示先装载镜像 tar。
  - `scanImageRefs({ root, fs })` → pull 模式镜像清单：逐行正则扫 `image:`（保持非 YAML 解析），扫描范围 5 个 compose（`docker-compose.yml`、`-infra.yml`、`-infra.dev.yml`、`-services.yml`、`-observability.yml`），跳过 `${VAR}` 引用；仅 `docker-compose.yml` 与 `-services.yml` 的 tag 替换为 `${IMAGE_TAG}`（`TAG_REPLACE_FILES` 语义逐字取自 `:111-114`）；`REGISTRY`/`BASE_REGISTRY` 内联插值 fallback（`:71-78`）。
  - 命令：`ops load-images [--mode=load|pull] [--tag <ver>] [--dry-run]`；`docker load`/`docker pull` 走 execFileSync 数组。
- [ ] **Step 1: 失败测试**：manifest 存在 → 按 manifest 顺序；manifest 缺失但有 tar → glob 模式列出全部 tar 且不抛；两者皆无 → GuardError；scanImageRefs 对含 `${IMAGE_TAG:-1.6.0}` 与 `image: redis:7` 混合 fixture 断言跳过变量行/保留字面 tag/tag 替换只发生在两个 TAG_REPLACE_FILES。
- [ ] **Step 2: 确认失败 → 实现 → 通过**。
- [ ] **Step 3: Commit** — `feat(cli): ops load-images with manifest-less glob fallback`

### Task 6: `coder ops upgrade`（就地升级 + 版本对账提示）

**Files:**
- Create: `packages/cli/lib/upgrade.js`
- Modify: `packages/cli/commands/ops.js`
- Test: `packages/cli/test/upgrade.test.js`

**Interfaces:**
- Produces: `validateTag(input)` → 剥 `v` 后 `^\d+\.\d+\.\d+$` 校验，失败 `UsageError`（语义照搬 `bin/upgrade.js:19-23`）；`planUpgrade({ root, fs, tag })` → `{ imageTag, envRewritten, currentImageTag, manifestVersion }`——`.env` 不存在 `GuardError`（提示先 `ops secrets`）；`IMAGE_TAG=` 改写走 Task 1 `rewriteEnvLine`（幂等）；manifestVersion 读 `MANIFEST.json.version`（无文件为 null）；执行序列 `compose pull` → `compose up -d`，env 注入 `IMAGE_TAG=<tag>`（进程环境优先于 --env-file 的既有机制不变，`bin/upgrade.js:6-10` 注释契约）。收尾提示跑 `ops verify`。
- [ ] **Step 1: 失败测试**：`v1.7.0`/`1.7.0` 均规约为 `1.7.0`；`1.7` 抛 UsageError；无 `.env` 抛 GuardError；`\r\n` fixture 双跑第二次 `envRewritten=false` 且字节不变；`.env` 无 `IMAGE_TAG` 行时追加；MANIFEST fixture 的 version 被读出。
- [ ] **Step 2: 确认失败 → 实现 → 通过**。
- [ ] **Step 3: Commit** — `feat(cli): ops upgrade with idempotent env rewrite and manifest echo`

### Task 7: dist-template 模板切换（bin/*.js 删除 + Makefile 转发）

**Files:**
- Delete: `release/dist-template/bin/*.js` 全部 10 个 + `release/dist-template/bin/start-deploy.sh.legacy`
- Modify: `release/dist-template/Makefile`（全量重写转发；删除 `build-frontend` 死 target——其引用的 `bin/build-frontend.js` 已随 server-only 重构删除）
- Modify: `release/dist-template/images/manifest.json`（`load_command` 改 `node cli/bin/coder.js ops load-images`；该静态占位清单本批不深改）
- Test: `packages/cli/test/dist-template.test.js`（新文件：防回归断言）

**Interfaces:**
- Produces: Makefile 11 个 target 全部 `@node cli/bin/coder.js ops <name> [$(vars)]`；`help` 文案同步（去掉 build-frontend 行；`up` 描述去掉「自动构建 frontend」）。`svc=`/`TAG=` 传参点各一（`logs`/`upgrade`）。
- 消费方：Task 8 的打包器接线使包内真的存在 `cli/`。

- [ ] **Step 1: 失败测试** `dist-template.test.js`：读 `release/dist-template/Makefile` 断言——11 个 target（无 `build-frontend`）每个 recipe 行匹配 `^	@node cli/bin/coder\.js ops `；`logs` 行含 `$(svc)`、`upgrade` 行含 `$(TAG)`；全文件无 `node bin/`。先跑确认 FAIL（现状是 `node bin/*.js`）。
- [ ] **Step 2: 删 11 个 bin 文件 + 重写 Makefile + images/manifest.json 的 load_command。**
- [ ] **Step 3: 验证**：`pnpm test:cli` 全绿；`grep -rn "node bin/" release/dist-template/{Makefile,docs,README.md,CHANGELOG.md}` 仅允许出现于「历史记录性」文本，操作性指令零残留（逐条人工核对清单写进 commit body）。
- [ ] **Step 4: Commit** — `feat(dist): switch template entrypoints to coder ops, drop legacy bin scripts`

### Task 8: 打包器最小接线（CLI 入包 + CI 远端命令切换）

**Files:**
- Modify: `release/dist-template/scripts/release.js`（阶段二骨架同步之后加一段：拷 `packages/cli/{bin,lib,commands,package.json}` → `DIST/cli/`，排除 test；约 10 行，标注批次三整体收编）
- Modify: `release/dist-template/scripts/release-deploy.js`（bin 白名单段 `:368-391` 改为同款 `cli/` 拷贝；动态生成 Makefile `:394-437` 的 target 转发行改 `node cli/bin/coder.js ops ...`，`up` 去掉已死的 `--deploy`；指引文案 `:473,588` 同步）
- Modify: `.gitlab-ci.yml` `deploy:staging`（远端 `node bin/load-images.js --mode=pull` → `node cli/bin/coder.js ops load-images --mode=pull`）
- Test: 手工冒烟（本任务无新单测；打包器自身测试属批次三）

**Interfaces:**
- Produces: airgap 包与 deploy 包内均含 `cli/bin/coder.js` 可执行（零依赖），包根 `Makefile` → `cli/` → `docker compose` 三层链路成立。

- [ ] **Step 1: 改三处。**
- [ ] **Step 2: 本地冒烟**：`node release/dist-template/scripts/release-deploy.js local-test --no-images` 产包后断言：`cli/bin/coder.js` 存在、`node cli/bin/coder.js doctor --json` 在包内报 `dist` 上下文、Makefile 无 `node bin/` 残留、`make help` 可跑（Git Bash）。
- [ ] **Step 3: Commit** — `feat(dist): bundle coder cli into packages and rewire staging deploy commands`

### Task 9: `coder init wizard`（setup-wizard 收编，prompts→readline）

**Files:**
- Create: `packages/cli/lib/wizard-config.js`（DEFAULTS/校验器/读写与 v3 迁移/env 解析，语义逐字照搬 `scripts/setup-wizard.js:95-129,174-224,301-463`）
- Create: `packages/cli/lib/wizard-render.js`（cargo/maven/npm 五文件渲染，照搬 `:697-872`；`DEFAULT_BUILDER_REGISTRY` 常量内联——解耦 `scripts/lib/Base-Images-Writer.js`）
- Create: `packages/cli/lib/wizard-ci-sync.js`（`CI_FIELD_MAP`/`normalizeLines`/`applyGitlabCI`/`syncGitlabCI`/`generateCILines` 逐字迁移 `:112-129,874-967`）
- Create: `packages/cli/lib/prompt.js`（`node:readline/promises` 零依赖实现 `ask({type:'text'|'password',message,initial,validate})`；stdin 非 TTY 直接短路返回 initial/undefined；Ctrl+C 退 130）
- Create: `packages/cli/commands/init.js`（注册 `init wizard`，交互向导 10 步照搬 `:505-695`；flags `--non-interactive|--defaults|--show`）
- Modify: `packages/cli/lib/commands.js`
- Test: `packages/cli/test/wizard-ci-sync.test.js`（**整体迁移 `scripts/setup-wizard.test.js` 的 5 个用例**，改 require 路径）、`packages/cli/test/wizard-config.test.js`、`packages/cli/test/prompt.test.js`

**Interfaces:**
- Produces: 写入契约与旧脚本字节级等价（消费方见 Global Constraints 行为等价锚点）；`loadConfig(root)`/`resolveConfig({existing, env, defaults})`（13 个 `CODER_*` env 字段级覆盖 + `CODER_SYNC_GITLAB_CI`）导出供测试与 `--show`。
- 迁移修正（收编坏实现的红线）：`--show` 保持零写入；文档曾提的 `--dry-run` 不实现（消除漂移，docs 由 Task 12 清）。

- [ ] **Step 1: 失败测试**：迁移用例全绿前置——先原样迁移 `setup-wizard.test.js` 断言在新路径 FAIL（模块不存在）；wizard-config：v3 `mavenToken` 迁移、字段级 env 覆盖混合、config v5 写读 roundtrip；prompt：非 TTY 短路、Ctrl+C 130（mock rl）。
- [ ] **Step 2: 确认失败 → 实现 → 通过**。
- [ ] **Step 3: 真仓冒烟**（`--show` + tmp clone 上 `--defaults` 跑一遍，`git status` 验只落预期文件）；`pnpm test:cli` 全绿。
- [ ] **Step 4: Commit** — `feat(cli): init wizard with readline prompts and byte-stable config writes`

### Task 10: `coder init dev` + `coder init check-env`（含三处修复）

**Files:**
- Create: `packages/cli/lib/check-env.js`
- Modify: `packages/cli/commands/init.js`
- Test: `packages/cli/test/check-env.test.js`、`packages/cli/test/init-dev.test.js`

**Interfaces:**
- Produces:
  - `checkEnv({ root, exec, probe })` → `{ http:[{name,url,ok,status}], docker:[{name,expected,actual,ok}], ok }`。**修复三处**：① 新文件无 BOM；② `docker compose`（去连字符，`execFileSync('docker',['compose',...])`）；③ docker 服务清单按 `infrastructure/docker-compose-services.yml` + `-infra.yml` 实况重写：infra `consul/redis/mysql/rabbitmq`（healthy）+ services `api-gateway/token-service/iam-service/notification-service/prue_backend/config-service/file-service/audit-service`（running，expectedStatus 照旧例 healthy/running 分配原则：infra 四项 healthy、业务项 running）；HTTP 探活清单 7 条逐字保留（`:3000` 网关路由 + consul 8500，`scripts/check-dev-env.js:5-13`——dev 网关 3000 不受 server-only 影响）。
  - `coder init dev [--reset]`：编排 `checkEnv → docker compose build → up -d → demo-data[--reset] → ps → 摘要`（步骤序、失败即停语义照搬 `scripts/dev-setup.js:24-66`；摘要端口段 `3000/6001-6007` 保留原文案）。demo-data 本体属批次三（C 桶 1052 行），本命令通过 `node scripts/demo-data.js` 过渡调用并在批次三收口——**此为唯一允许的 scripts/ 过渡引用**，在代码注释与 Task 12 验证清单里显式登记。

- [ ] **Step 1: 失败测试**：check-env fixture（compose ps 输出桩）断言新服务清单 13 项与 expectedStatus 分配；一项 HTTP fail → ok=false；docker 命令是 `['compose',...]` 数组（非字符串）。init-dev：fakeExec 断言步骤序与失败短路。
- [ ] **Step 2: 确认失败 → 实现 → 通过**。
- [ ] **Step 3: Commit** — `feat(cli): init dev and check-env with refreshed service inventory`

### Task 11: `coder init pull-images`

**Files:**
- Create: `packages/cli/lib/pull-images.js`
- Modify: `packages/cli/commands/init.js`
- Test: `packages/cli/test/pull-images.test.js`

**Interfaces:**
- Produces: `planPulls({ root, fs })` → `{ items:[{mirrorRef,ref,useMirror}], manifestMissing:boolean }`（读 `infrastructure/base-images.json` 策略，mirror 优先双拉、mirror 失败继续源 ref——语义照搬 `scripts/images-pull.js:39-62`）；manifest 缺失 `GuardError`（exit 3，提示先 `images:sync`——退出码语义从旧 2 调整入统一表）。命令 flags `--dry-run`。
- [ ] **Step 1: 失败测试**：有 mirror 项生成双 ref；`--dry-run` 零 docker 调用；manifest 缺失 GuardError。
- [ ] **Step 2: 确认失败 → 实现 → 通过。**
- [ ] **Step 3: Commit** — `feat(cli): init pull-images absorbing images-pull script`

### Task 12: 收编清理、发布流卫生与文档清扫

**Files:**
- Delete: `scripts/setup-wizard.js`、`scripts/setup-wizard.test.js`、`scripts/dev-setup.js`、`scripts/check-dev-env.js`、`scripts/images-pull.js`
- Modify: `package.json`（根 scripts：`setup:wizard`/`setup` 链/`dev:setup`/`check:env`/`images:pull` 改 coder 薄封装；`test:scripts` 删除并入 `test:cli`；`setup` 链中 `images:sync` 保留原样——base-images-sync 属后续批次）
- Modify: `packages/cli/lib/rust-publish.js`（**缺陷修复**：path 依赖改写发布前快照 `Cargo.lock`，try/finally 一并复原——当前发布流程会在仓里残留 kellnr sparse registry 条目污染锁文件，2026-10-31 工作区实证：`digua_common_error 1.6.0 (sparse+http://192.168.2.12:16444/...)` 双版本并存）
- Modify: `.gitignore`（加 `/release/compose-template/`——`coder release compose-template` 本地产物目录）
- Modify: `.trae/skills/coder-dev-bootstrap/SKILL.md:25`（check-dev-env 路径改 `coder init check-env`）
- Modify: `docs-site/guides/docker-registry-config.md`、`docs-site/user-manual/01-quick-start.md`、`docs-site/guides/rust-sdk-publishing.md`、`docs-site/ai-context.md`、`docs-site/user-manual/12-release-and-distribute.md`、`release/dist-template/{README.md,docs/deployment/*.md}`（操作性 `node scripts/setup-wizard|dev-setup|check-dev-env|images-pull` 与 `node bin/*` 引用改 coder 等价命令；`--dry-run` 漂移文档删除）
- Test: `packages/cli/test/rust-publish.test.js`（扩展锁复原用例）

**Interfaces:**
- Consumes: Task 2-7/9-11 的命令面。
- Produces: 根 npm scripts 收缩约 5 条；`node scripts/` 在 docs-site/dist-template/docs/skills 的操作性引用对本批收编对象清零。

- [ ] **Step 1: 失败测试**：rust-publish 注入 fakeExec+真实 tmp 仓（含 Cargo.lock fixture）跑发布序列，断言结束后 `Cargo.lock` 字节等于快照。
- [ ] **Step 2: 删 5 个脚本 + 改 package.json/.gitignore/rust-publish + 文档清扫。**
- [ ] **Step 3: 全量验证**：`pnpm test:cli` 全绿；`node packages/cli/bin/coder.js doctor --json` 上下文与命令面正确；`grep -rn "setup-wizard\|dev-setup\|check-dev-env\|images-pull" package.json docs-site/ release/dist-template/docs .trae/skills .agents/skills .gitlab-ci.yml` 仅剩历史记录性文本（逐条核对清单入 commit body）；工作区无 Cargo.lock 漂移。
- [ ] **Step 4: Commit** — `chore: retire batch-2 absorbed scripts, restore lock hygiene and refresh docs`

---

## 批次二的完成判据

1. `pnpm test:cli` 全绿（新增 ≥10 个测试文件），`test:all` 不变绿转红。
2. 分发包（本地 `release-deploy.js local-test --no-images` 产物）内：`make help/secrets/up/verify/upgrade` 的命令面与语义等价可走通（CLI 本体在 `cli/`、`doctor` 报 dist、无 `node bin/` 残留）；airgap 包 `make load-images` 在无 manifest 时降级 glob 成功。
3. 上游仓：`coder init wizard --defaults` 在 tmp clone 产出的五类配置文件与旧脚本输出字节级等价（CI 同步幂等用例迁移后全绿）；`coder init check-env` 服务清单与当前 compose 实况一致。
4. 侦察实证五缺陷全部关闭：dist 上下文判定、verify 端口表、airgap manifest、Makefile 死 target、check-dev-env 三处；外加 rust-publish 锁污染修复有测试锁定。
5. 全仓 `node scripts/` 操作性引用对本批收编对象清零；根 npm scripts 净减 ≥5 条；`release compose-template` 命令与其产物目录未被触碰。
