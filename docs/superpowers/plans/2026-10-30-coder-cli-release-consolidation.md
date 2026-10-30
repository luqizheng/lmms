# @digua/cli 骨架与发布链路收敛 实施计划（批次一）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立 `packages/cli`（bin `coder`）骨架与输出/上下文层，把版本账本（status/bump/tag）与四路发布（镜像、Maven、kellnr、npm 包）从 `scripts/*.js` 收编为 `coder` 子命令，切换 6 个 CI job，删除被收编的旧脚本。

**Architecture:** 单个零依赖 CJS 包。`lib/cli.js` 按「长命令名」做表驱动分发，每个命令返回统一 Result（data/human/exitCode/warnings），由 `lib/output.js` 渲染为人类输出或单行 JSON 信封。业务逻辑放进可注入 `exec`/`fetchImpl` 的 lib 模块，命令层只做参数解析与编排——这样纯逻辑可单测，外部命令失败可模拟。

**Tech Stack:** Node.js ≥18，CommonJS，`node:test` + `node:assert/strict`，无运行时依赖（js-yaml 属批次四，本批不引入）。

**Spec:** `docs/superpowers/specs/2026-10-30-coder-cli-engineering-shell-design.md`

## Global Constraints

- Node ≥ 18，CommonJS（`packages/cli/package.json` 不写 `"type"`），**运行时零依赖**。
- 所有命令支持 `--json`；JSON 模式下 **stdout 只允许一行 JSON**，一切人类可读日志走 stderr。
- 退出码表固定：`0` 成功 / `1` 未分类失败 / `2` 用法错误 / `3` 前置条件不满足（脏工作树、版本漂移、资产缺失、registry 不可达）/ `4` 外部命令失败（git/docker/cargo/mvn/npm 非零退出）。
- **coder 永不执行 `git push`、永不自动创建 tag**；`release tag` 只建本地 tag 并打印推送命令。
- **版本权威源 = git tag**（`v` 前缀）。Makefile IMAGE_TAG 与 CHANGELOG 标题不带 `v`。
- 平台：开发主机 Windows（Git Bash）+ CI Linux。只用 node API 与 `execFileSync`，不拼 bash 字符串，不硬编码 `/` 路径拼接。
- 被收编的脚本一律**删除**，不做转发壳；旧入口只保留根 package.json 的 `coder` 一条。
- 仓库硬规则：中文文档、英文 commit、一任务一 commit、禁 amend/force-push。

## Review Focus

1. **CRLF 输入**：Makefile/CHANGELOG 改写用 `^...$` 多行正则，Windows checkout 可能带 `\r\n`；行尾必须原样保留，不得把整个文件转成 LF 或留下 `\r` 在替换值里。
2. **非 TTY / CI**：交互式提示必须降级为默认值而非挂起；`--json` 下 spinner 与非 TTY 日志不得写进 stdout。
3. **空版本状态**：浅克隆、无 `v*` tag 的仓库（或 `git` 不可用）下 `release status` 必须报告 `null` 并以漂移退出码结束，不能抛栈。
4. **发布中途失败的回滚**：rust 发布要临时把 `path` 依赖改写成版本依赖，任一 crate 失败都必须 `try/finally` 复原 `Cargo.toml`。
5. **stdout 纯净性**：任何 `console.log` 泄漏都会破坏 agent 的 JSON 解析——由抓取 stdout 的测试锁死。

---

### Task 1: CLI 骨架、输出层与上下文探测

**Files:**
- Create: `packages/cli/package.json`
- Create: `packages/cli/bin/coder.js`
- Create: `packages/cli/lib/errors.js`
- Create: `packages/cli/lib/output.js`
- Create: `packages/cli/lib/context.js`
- Create: `packages/cli/lib/repo.js`
- Create: `packages/cli/lib/commands.js`
- Create: `packages/cli/lib/cli.js`
- Create: `packages/cli/test/cli.test.js`
- Create: `packages/cli/test/context.test.js`
- Modify: `pnpm-workspace.yaml`（packages 列表追加 `"packages/cli"`）
- Modify: `package.json`（根 scripts 加 `"coder": "node packages/cli/bin/coder.js"`、`"test:cli": "node --test packages/cli/test/"`）

**Interfaces:**
- Produces:
  - `lib/errors.js`：`EXIT = { OK: 0, FAIL: 1, USAGE: 2, GUARD: 3, EXTERNAL: 4 }`；`class CoderError extends Error { constructor(code, message, details = {}) }`，实例带 `this.code`（字符串，如 `'USAGE'`）与 `this.exitCode`；子类 `UsageError`(exitCode 2)、`GuardError`(3)、`ExternalError`(4)。
  - `lib/output.js`：`createOutput({ json, stdout, stderr })` → `{ json, log(msg), warn(msg), error(msg), emit({ command, result }) }`。`emit` 在 json 模式向 stdout 打一行 `{ command, ok, exitCode, data, warnings, errors }`；人类模式向 stdout 打 `result.human` 各行。非 json 模式下 `log/warn/error` 都写 stdout，json 模式一律写 stderr。
  - `lib/context.js`：`detectContext(cwd) -> { kind: 'monorepo'|'dist'|'unknown', root: string|null }`。判定规则：`Cargo.toml` + `services/` + `libs/` 同时存在 → `monorepo`；`images/manifest.json` + `docker-compose.yml` + `Makefile` 同时存在 → `dist`；否则 `unknown`。`detectContext` 从 `cwd` 逐级向上找。
  - `lib/repo.js`：`{ findRoot(cwd) }`（复用 `detectContext`，返回 root 或 null）、`git(root, args) -> string`（`execFileSync`，失败抛 `ExternalError`）、`dirtyCount(root)`、`currentBranch(root)`。`latestVersion` 属 Task 3。
  - `lib/commands.js`：`COMMANDS`，键为完整命令名（空格分隔，如 `'release status'`），值为 `{ summary, run: async (args, ctx) => Result }`。本任务只注册 `'version'`。
  - `lib/cli.js`：`run(argv, { cwd = process.cwd(), stdout, stderr } = {}) -> Promise<number>`。已解析的全局 flag：`--json`、`--cwd <dir>`、`--help`、`--version`。`Result = { data: object, human: string[]|string, exitCode?: number, warnings?: string[] }`。分发规则：取 argv 中最长的已注册命令键前缀，剩余参数原样传给 `run`；无匹配 → `UsageError`。
- 消费方（后续任务）：所有命令模块 `require('../lib/errors.js' | '../lib/output.js')`，只从 `ctx` 取 `{ root, context, out, json }`。

- [ ] **Step 1: 写失败测试**

`packages/cli/test/cli.test.js`：
```js
const test = require('node:test');
const assert = require('node:assert/strict');
const { run } = require('../lib/cli.js');
const { EXIT } = require('../lib/errors.js');

function capture() {
  const out = { text: '', write(s) { this.text += s; return true; } };
  const err = { text: '', write(s) { this.text += s; return true; } };
  return { stdout: out, stderr: err };
}

test('--version prints package version and exits 0', async () => {
  const io = capture();
  const code = await run(['--version'], { ...io });
  assert.equal(code, EXIT.OK);
  assert.match(io.stdout.text.trim(), /^\d+\.\d+\.\d+/);
});

test('unknown command exits USAGE with json envelope on stdout only', async () => {
  const io = capture();
  const code = await run(['bogus', '--json'], { ...io });
  assert.equal(code, EXIT.USAGE);
  const lines = io.stdout.text.trim().split('\n');
  assert.equal(lines.length, 1);
  const env = JSON.parse(lines[0]);
  assert.equal(env.command, 'cli');
  assert.equal(env.ok, false);
  assert.equal(env.exitCode, EXIT.USAGE);
  assert.equal(env.errors[0].code, 'USAGE');
  assert.equal(io.stderr.text, '');
});

test('json mode keeps human logs off stdout', async () => {
  const io = capture();
  await run(['version', '--json'], { ...io });
  const env = JSON.parse(io.stdout.text.trim());
  assert.equal(env.command, 'version');
  assert.equal(env.ok, true);
  assert.equal(typeof env.data.node, 'string');
});
```

`packages/cli/test/context.test.js`：
```js
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { detectContext } = require('../lib/context.js');

function fixture(files) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-ctx-'));
  for (const [rel, content] of Object.entries(files)) {
    const p = path.join(dir, rel);
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, content);
  }
  return dir;
}

test('detects monorepo by Cargo.toml + services + libs', () => {
  const dir = fixture({ 'Cargo.toml': '[workspace]', 'services/.keep': '', 'libs/.keep': '' });
  assert.equal(detectContext(path.join(dir, 'services')).kind, 'monorepo');
  assert.equal(detectContext(dir).root, dir);
});

test('detects dist by manifest + compose + Makefile', () => {
  const dir = fixture({ 'images/manifest.json': '{}', 'docker-compose.yml': 'x', Makefile: 'IMAGE_TAG ?= 1.0.0' });
  assert.equal(detectContext(dir).kind, 'dist');
});

test('unknown context has null root', () => {
  const dir = fixture({ 'README.md': 'x' });
  assert.deepEqual(detectContext(dir), { kind: 'unknown', root: null });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `node --test packages/cli/test/`
Expected: FAIL，`Cannot find module '../lib/cli.js'`

- [ ] **Step 3: 实现骨架**

`packages/cli/package.json`：
```json
{
  "name": "@digua/cli",
  "version": "0.0.0",
  "description": "coder — low-memory-micro-services 工程化外壳 CLI",
  "bin": { "coder": "./bin/coder.js" },
  "main": "./lib/cli.js",
  "files": ["bin", "lib", "commands"],
  "engines": { "node": ">=18" },
  "scripts": { "test": "node --test test/" },
  "publishConfig": { "registry": "http://npm.kudi.repos/" }
}
```

`bin/coder.js`：shebang + `require('../lib/cli.js').run(process.argv.slice(2))`，把返回值赋给 `process.exitCode`；捕获未处理异常时打印 `err.stack` 到 stderr 并置 `process.exitCode = 1`。

`lib/errors.js`：按 Interfaces 实现四个类，`CoderError` 保存 `details`。

`lib/output.js`：按 Interfaces 实现；`emit` 的 json 分支用 `JSON.stringify`，`errors` 从 `result.errors ?? []` 取。

`lib/context.js`：`detectContext` 向上遍历，按 monorepo → dist 顺序判定。

`lib/repo.js`：`git()` 用 `execFileSync('git', args, { cwd, encoding: 'utf-8' })`，`ExternalError` 包装非零退出且带上 stderr 首行。

`lib/commands.js`：注册 `'version'`，`run` 返回 `{ data: { node: process.version, cli: require('../package.json').version }, human: [...] }`。

`lib/cli.js`：解析全局 flag（`--json`、`--cwd`），最长前缀匹配 `COMMANDS`，构造 `ctx = { root, context, out, json }` 传给 handler，异常经 `toEnvelope` 统一渲染（`CoderError` 用自身 exitCode，其它用 `EXIT.FAIL`）。

- [ ] **Step 4: 运行测试确认通过**

Run: `node --test packages/cli/test/`
Expected: PASS（5 个用例）

- [ ] **Step 5: 提交**

```bash
git add packages/cli pnpm-workspace.yaml package.json
git commit -m "feat(cli): scaffold coder CLI with output envelope and context detection"
```

---

### Task 2: 版本、Makefile 与 CHANGELOG 账本

**Files:**
- Create: `packages/cli/lib/version.js`
- Create: `packages/cli/lib/makefile.js`
- Create: `packages/cli/lib/changelog.js`
- Create: `packages/cli/test/version.test.js`
- Create: `packages/cli/test/makefile.test.js`
- Create: `packages/cli/test/changelog.test.js`

**Interfaces:**
- Produces:
  - `lib/version.js`：`normalizeVersion(input) -> { tag, image, parts } | null`；`compareVersions(a, b) -> number`；`bumpVersion(ver, level) -> { tag, image, parts }`，`level ∈ {'major','minor','patch'}`；`latestTag(gitTagOutput) -> version | null`。语义照搬 main 分支 `.wt-release-cli/release/release-cli/lib/version.js`（`v1.5.1` ⇒ `{tag:'v1.5.1', image:'1.5.1'}`；非 semver tag 忽略）。
  - `lib/makefile.js`：`readImageTag(text) -> string|null`；`applyVersion(text, imageTag) -> string`（幂等，同时改 `IMAGE_TAG ?= x` 行与 `coder-micro-services vX` 横幅）；`bumpMakefileFile(filePath, imageTag) -> boolean`（内容变化返回 true）。语义照搬上述 `lib/makefile.js`，**但行尾必须保持原样**。
  - `lib/changelog.js`：`readTopVersion(text) -> string|null`（首个 `## vX.Y.Z` 的 X.Y.Z）；`hasVersion(text, imageTag) -> boolean`；`insertEntry(text, { version, date, body }) -> string`（插在首个 `## v` 之前；无既有条目则接在 `# Changelog` 标题后）。
- 消费方：Task 3/4。

- [ ] **Step 1: 写失败测试**

`test/version.test.js`（从 `.wt-release-cli/release/release-cli/lib/version.test.js` 移植断言，至少覆盖）：`normalizeVersion('v1.5.1')` 与 `('1.5.1')` 同值；`normalizeVersion('1.5')` 为 `null`；`bumpVersion(x,'minor')` 把 patch 归零（`1.5.1` → `1.6.0`）；`latestTag` 忽略 `archive/demo-data-fresh-db-register` 只取最大 `v*`。

`test/makefile.test.js`：
```js
const test = require('node:test');
const assert = require('node:assert/strict');
const { readImageTag, applyVersion } = require('../lib/makefile.js');

const LF = 'IMAGE_TAG ?= 1.0.0\nhelp:\n\t@echo "coder-micro-services v1.0.0 — 可用命令："\n';

test('reads and rewrites LF makefile', () => {
  assert.equal(readImageTag(LF), '1.0.0');
  const out = applyVersion(LF, '1.7.0');
  assert.match(out, /IMAGE_TAG \?= 1\.7\.0/);
  assert.match(out, /coder-micro-services v1\.7\.0/);
  assert.equal(applyVersion(out, '1.7.0'), out, 'idempotent');
});

test('preserves CRLF line endings', () => {
  const CRLF = LF.replaceAll('\n', '\r\n');
  const out = applyVersion(CRLF, '1.7.0');
  assert.ok(out.includes('IMAGE_TAG ?= 1.7.0\r\n'));
  assert.ok(!/[^\r]\n/.test(out), 'no bare LF introduced');
});
```

`test/changelog.test.js`：
```js
const test = require('node:test');
const assert = require('node:assert/strict');
const { readTopVersion, hasVersion, insertEntry } = require('../lib/changelog.js');

const DOC = '# Changelog\n\n## v1.0.0 (2026-07-12)\n\n### Added\n- 初始版本\n';

test('reads top version and tests membership', () => {
  assert.equal(readTopVersion(DOC), '1.0.0');
  assert.equal(hasVersion(DOC, '1.0.0'), true);
  assert.equal(hasVersion(DOC, '1.7.0'), false);
});

test('inserts new entry above existing ones', () => {
  const out = insertEntry(DOC, { version: '1.7.0', date: '2026-10-30', body: '### Added\n- coder CLI' });
  assert.equal(readTopVersion(out), '1.7.0');
  assert.ok(out.startsWith('# Changelog\n\n## v1.7.0 (2026-10-30)\n\n### Added\n- coder CLI\n\n## v1.0.0'));
});

test('keeps CRLF documents CRLF', () => {
  const out = insertEntry(DOC.replaceAll('\n', '\r\n'), { version: '1.7.0', date: '2026-10-30', body: '- x' });
  assert.ok(!/[^\r]\n/.test(out));
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `node --test packages/cli/test/`
Expected: FAIL，`Cannot find module '../lib/version.js'`

- [ ] **Step 3: 实现三个模块**

`version.js` 直接移植 release-cli 版本（语义已被 17 个单测验证过）。`makefile.js` 移植时把正则改为对 CRLF 安全：读取到 `\r` 时先记录 `const eol = text.includes('\r\n') ? '\r\n' : '\n'`，替换时保留原 `\r`（用 `[^\r\n]*` 匹配行内内容，不要用 `$` 后接 `.`）。`changelog.js` 用 `^##\s+v?(\d+\.\d+\.\d+)` 多行匹配；`insertEntry` 同样按探测到的 EOL 拼接。

- [ ] **Step 4: 运行测试确认通过**

Run: `node --test packages/cli/test/`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add packages/cli/lib packages/cli/test
git commit -m "feat(cli): add version, makefile and changelog ledger helpers"
```

---

### Task 3: `coder release status` 与 `coder doctor`

**Files:**
- Create: `packages/cli/lib/release-status.js`
- Create: `packages/cli/commands/release.js`
- Create: `packages/cli/test/release-status.test.js`
- Modify: `packages/cli/lib/commands.js`（注册 `release` 模块）

**Interfaces:**
- Produces:
  - `lib/release-status.js`：`collectStatus({ root, git, dirty, branch }) -> { git: { latestTag, imageTag, dirty, branch }, makefile: { path, imageTag }, changelog: { path, version }, consistent, drift: string[] }`。`git`/`dirty`/`branch` 均为可注入函数（默认取 `lib/repo.js` 的实现）。`drift` 元素取自 `['git','makefile','changelog']`，指「与 git tag 不一致的账本」；三处皆 `null` 时 `consistent: true`、`drift: []`（全新仓库无账可对）。任一非 null 但不相等 → 计入 drift。
  - `lib/commands.js` 改为单一注册表：逐个 `require('../commands/*.js')` 并 `Object.assign` 合并其 `commands` 键。本任务新增 `commands/release.js`，导出 `{ commands: { 'release status': { summary, run }, 'doctor': { summary, run } } }`；后续任务的命令模块一律按同一形状导出并被注册。
- `release status` 退出码：`0` 一致；`3` 有漂移（`GuardError` 不抛出，直接返回 `exitCode: EXIT.GUARD`）；`latestTag` 为 `null` 时同样退 `3` 并在 `warnings` 说明。

- [ ] **Step 1: 写失败测试**

`test/release-status.test.js`：用 `fs.mkdtempSync` 造 fixture，`git` 依赖注入：
```js
const status = collectStatus({
  root: dir,
  git: () => 'v1.6.0\narchive/demo-data-fresh-db-register',   // 假 git tag -l
  dirty: () => 1,
  branch: () => 'feat/x',
});
assert.deepEqual(status.drift.sort(), ['changelog', 'makefile']);
assert.equal(status.consistent, false);
```
补充用例：三处一致（Makefile/CHANGELOG 都 1.6.0）→ `consistent: true`；`git` 抛 `ExternalError` → `latestTag: null`、`drift` 含三项、不抛栈。

- [ ] **Step 2: 运行测试确认失败**

Run: `node --test packages/cli/test/release-status.test.js`
Expected: FAIL，模块不存在

- [ ] **Step 3: 实现**

`collectStatus` 读取 `<root>/release/dist-template/Makefile` 与 `<root>/release/dist-template/CHANGELOG.md`（缺失时对应字段 `null` 并入 drift）。命令层把 `data` 原样放进 Result，`human` 输出三行对账表 + 结论行；漂移时 `warnings` 列出「以 git tag 为准执行 `coder release bump --force --from-tag`」。

`doctor`：输出 `{ context, root, node, status }`，`human` 打印上下文与命令可用性提示（本批 monorepo 只列 release/build/assets）。

- [ ] **Step 4: 运行测试确认通过**

Run: `node --test packages/cli/test/` 与 `node packages/cli/bin/coder.js release status --json`
Expected: 测试 PASS；真实仓库输出 `consistent:false`、`drift` 含 `makefile` 与 `changelog`、退出码 3

- [ ] **Step 5: 提交**

```bash
git add packages/cli
git commit -m "feat(cli): add release status and doctor commands"
```

---

### Task 4: `coder release bump` 与 `coder release tag`

**Files:**
- Create: `packages/cli/lib/release-bump.js`
- Create: `packages/cli/test/release-bump.test.js`
- Modify: `packages/cli/commands/release.js`
- Modify: `packages/cli/lib/commands.js`

**Interfaces:**
- Produces:
  - `lib/release-bump.js`：
    - `resolveTarget({ root, level, version, force, fromTag, status }) -> { version, imageTag }`：`--version X` 优先；否则以 `status.git.latestTag` 为基线按 `--level` 递增；漂移未解决且无 `--force --from-tag` → 抛 `GuardError('VERSION_DRIFT', ...)`。
    - `bumpLedger({ root, imageTag, changelogBody, date, fs = nodeFs }) -> { changed: string[] }`：改写 Makefile 与 CHANGELOG，返回被改文件相对路径。
    - `commitLedger({ root, imageTag, exec = defaultExec }) -> { commit }`：只 `git add release/dist-template/Makefile release/dist-template/CHANGELOG.md` + `git commit -m "chore(release): bump version ledger to <imageTag>"`。
  - `release tag [<version>] [--dry-run]`：前置校验（工作树干净、三处一致、tag 不存在），`git tag -a vX.Y.Z -m "vX.Y.Z"`，返回 `{ tag, pushed: false, next: 'git push origin main vX.Y.Z' }`；人类输出必须显式打印「尚未推送，需人工确认后执行 …」。
- 命令层 `bump` 参数：`--level patch|minor|major`（默认 `minor`）、`--version <x.y.z>`、`--changelog <file>`（必填，正文来源）、`--force --from-tag`、`--dry-run`。

- [ ] **Step 1: 写失败测试**

`test/release-bump.test.js`：在临时目录造三份账本，注入假 `exec` 记录调用：
- `resolveTarget` 在 drift 且无 force 时抛 `CoderError` 且 `exitCode === 3`
- `resolveTarget({ version: 'v1.7.0' })` 归一化为 `{ version:'1.7.0', imageTag:'1.7.0' }`
- `--level minor` 从 `1.6.0` 得 `1.7.0`
- `bumpLedger` 只改两个文件，`changed` 恰好 `['release/dist-template/Makefile','release/dist-template/CHANGELOG.md']`
- `commitLedger` 的 exec 调用序列为两条、第二条含 `--no-verify` 之外的固定消息文本；**断言序列中不含 `push`**

- [ ] **Step 2: 运行测试确认失败**

Run: `node --test packages/cli/test/release-bump.test.js`
Expected: FAIL，模块不存在

- [ ] **Step 3: 实现**

按 Interfaces 实现。`bump` 的 `--changelog <file>` 读入正文后交给 `insertEntry`；`--dry-run` 时 `data.wouldChange` 列出文件而不落盘、不 commit。`tag` 在 `--dry-run` 下只返回将要创建的 tag。

- [ ] **Step 4: 运行测试确认通过**

Run: `node --test packages/cli/test/`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add packages/cli
git commit -m "feat(cli): add release bump and tag commands with guard rails"
```

---

### Task 5: `coder assets download`（swagger-ui 资产）

**Files:**
- Create: `packages/cli/lib/assets.js`
- Create: `packages/cli/test/assets.test.js`
- Modify: `packages/cli/lib/commands.js`（注册 `assets` 模块）
- Create: `packages/cli/commands/assets.js`

**Interfaces:**
- Produces:
  - `lib/assets.js`：`SWAGGER = { VERSION: 'v5.17.14', ZIP_NAME: 'swagger-ui-v5.17.14.zip', REMOTE_URL: 'https://gh-proxy.org/https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip', SHA256: '481244d0812097b11fbaeef79f71d942b171617f9c9f9514e63acbe13e71ccdc' }`（逐字取自 `scripts/download-swagger-ui.js`）；`verifySha256(buffer, expectedHex) -> boolean`；`ensureAsset({ root, mode, fetchImpl, fs }) -> { path, existed, downloaded, sha256 }`，`mode ∈ {'ensure','check','force'}`；`check` 缺失时抛 `GuardError('ASSET_MISSING')`。
  - 目标路径：`<root>/infrastructure/vendor/<ZIP_NAME>`。
- 命令：`coder assets download [--check] [--force]`，默认 `ensure`。

- [ ] **Step 1: 写失败测试**

用本地临时文件与假 `fetchImpl`（返回已知字节数组）：
- `verifySha256(Buffer.from('abc'), <正确 sha256>)` 为 `true`，错值为 `false`
- `ensureAsset` 在文件已存在且 `mode:'ensure'` 时不调用 `fetchImpl`，返回 `existed: true`
- `mode:'check'` 缺失时抛 `GuardError` 且 `exitCode === 3`
- 下载后 sha256 不匹配 → 抛 `GuardError('ASSET_SHA_MISMATCH')` 且不落盘

- [ ] **Step 2: 运行测试确认失败**

Run: `node --test packages/cli/test/assets.test.js`
Expected: FAIL，模块不存在

- [ ] **Step 3: 实现**

下载走 `node:https` 的 `get`，用 `createHash('sha256')` 校验后写盘；spinner 用 `out.log`（非 TTY 直接单行日志）。

- [ ] **Step 4: 运行测试确认通过**

Run: `node --test packages/cli/test/`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add packages/cli
git commit -m "feat(cli): absorb swagger-ui asset download as coder assets"
```

---

### Task 6: `coder build images`（吸收 build-docker.js）

**Files:**
- Create: `packages/cli/lib/docker-build.js`
- Create: `packages/cli/commands/build.js`
- Create: `packages/cli/test/docker-build.test.js`
- Modify: `packages/cli/lib/commands.js`

**Interfaces:**
- Produces:
  - `lib/docker-build.js`：
    - `SERVICES = ['api-gateway','token-service','iam-service','notification-service','file-service','config-service','audit-service','prue-admin-backend','docs-site']`（逐字取自 `scripts/build-docker.js` 现有清单）
    - `DEFAULT_REGISTRY = 'zhcoder-docker-registry.com:8000'`
    - `parseBuildArgs(argv) -> { services, version, push, loadAndPush, platform, prune }`（纯函数；位置参数只认 `SERVICES` 成员，`prune` 作为独立模式；`--platform` 默认 `linux/amd64`）
    - `resolvePlan({ args, isTTY }) -> { services, tags, registry, mode: 'push'|'load-and-push'|'local', warnings }`；非 TTY 且无版本号时沿用现有默认（无版本、不推送、全量服务）；多架构 + `loadAndPush` → `mode:'push'` 并给出 warning（沿用现有语义）
    - `buildImages({ root, plan, exec, out }) -> { built: [{ service, tags }], failed: [] }`
- 命令：`coder build images [services...] [-v|--version X] [--push|--load-and-push] [--platform a,b] | coder build images prune`。镜像名 `micro-server/<service>`，tag 同时打 `<ver>` 与 `latest`。

- [ ] **Step 1: 写失败测试**

`test/docker-build.test.js` 覆盖（表驱动）：
- `parseBuildArgs(['api-gateway','token-service','-v','1.6.0','--push'])` → services 两个、version `1.6.0`、push `true`
- 位置参数里的未知名字被忽略（与现有 `pickTargets` 语义一致）
- `resolvePlan({ args:{...}, isTTY:false })` 在无 version 时 `mode:'local'`、services 为全量
- `--load-and-push --platform linux/amd64,linux/arm64` → `mode:'push'` 且 `warnings` 非空
- `buildImages` 用注入 `exec` 断言 docker 命令序列包含 `buildx build`、每个 service 两条 tag、末尾无 `push`（`mode:'local'` 时）

- [ ] **Step 2: 运行测试确认失败**

Run: `node --test packages/cli/test/docker-build.test.js`
Expected: FAIL，模块不存在

- [ ] **Step 3: 实现**

逐段搬移 `scripts/build-docker.js` 的常量与命令拼装（`buildx build --platform … -t … -f … <ctx>`），交互式提示换成 `node:readline` 的极简选择（非 TTY 直接跳过）。`prune` 分支执行 `docker buildx prune -af`。docs-site 的宿主机 vitepress 预构建逻辑原样保留（它依赖 `pnpm docs:build`），并把失败归入 `failed` 而非中断其余服务。

- [ ] **Step 4: 运行测试确认通过**

Run: `node --test packages/cli/test/` 与 `node packages/cli/bin/coder.js build images --help`
Expected: 测试 PASS；help 输出中 flag 与现有脚本一致

- [ ] **Step 5: 提交**

```bash
git add packages/cli
git commit -m "feat(cli): absorb docker image build as coder build images"
```

---

### Task 7: `coder release publish maven`

**Files:**
- Create: `packages/cli/lib/maven-publish.js`（自 `scripts/lib/maven-publish.js` 移植）
- Create: `packages/cli/test/maven-publish.test.js`
- Modify: `packages/cli/commands/release.js`（加 `'release publish maven'`）

**Interfaces:**
- Produces：
  - `validateMavenVersion({ version, kind }) -> { version }`，`kind:'release'` 时含 `-SNAPSHOT` → `UsageError('MAVEN_VERSION_SNAPSHOT')`
  - `publishMaven({ root, version, kind, profile, dryRun, exec, env }) -> { modules, uploaded }`：步骤固定为 ① `mvn versions:set -DnewVersion=<v>`（幂等）② `mvn install` ③ 逐模块 `curl -T` 上传到 Reposilite。凭证解析顺序保持现状：CLI 参数 > env > setup-wizard 配置 > `~/.m2/settings.xml`。
  - 与现状一致的收尾提示：pom.xml 改动**不自动 commit**，`warnings` 里给出 `git add sdk/java && git commit -m "feat(sdk): bump to <ver>"`。
- 命令：`coder release publish maven --version <x.y.z> [--kind release|snapshot] [--profile <p>] [--dry-run]`，`--kind` 默认 `release`。

- [ ] **Step 1: 写失败测试**

- release + `-SNAPSHOT` → `UsageError`，`exitCode === 2`
- snapshot + `-SNAPSHOT` → 通过
- `publishMaven` 注入 `exec` 的调用序列：`versions:set` 早于 `install`，上传条数等于 `modules.length`；`dryRun` 时序列为空且返回 `wouldRun`
- 凭证优先级：`{ CLI: 'a', env: { MAVEN_USERNAME: 'b' } }` → 取 `a`

- [ ] **Step 2: 运行测试确认失败 → Step 3: 实现 → Step 4: 测试通过**

Run: `node --test packages/cli/test/maven-publish.test.js`
Expected: FAIL 后 PASS

- [ ] **Step 5: 提交**

```bash
git add packages/cli
git commit -m "feat(cli): absorb maven sdk publishing as coder release publish maven"
```

---

### Task 8: `coder release publish rust`

**Files:**
- Create: `packages/cli/lib/rust-publish.js`（合并 `scripts/publish-libs.js` + `scripts/publish-rust-sdks.js`）
- Create: `packages/cli/test/rust-publish.test.js`
- Modify: `packages/cli/commands/release.js`

**Interfaces:**
- Produces:
  - `LIBS_ORDER`（17 个 crate，Layer 0→2 拓扑，逐字取自 `publish-libs.js`）与 `SDKS_ORDER`（`digua-http-client` 在前 + 6 个 service SDK，取自 `publish-rust-sdks.js`）
  - `rewritePathDeps(tomlText, { crateVersions }) -> string`：把 `path = "../common-error"` 形态替换为 `version = "<v>"`，其余行保持不变
  - `publishRust({ root, version, registry = 'kellnr', targets = 'all'|'libs'|'sdks', dryRun, exec, fs }) -> { published: [{ name, version }] }`
  - **回滚契约**：所有 `Cargo.toml` 改写包在 `try/finally` 中，`finally` 用原始文本复原；任一步失败时抛 `ExternalError`，但文件已复原。
- 命令：`coder release publish rust <version> [--dry-run] [--registry kellnr] [--targets all|libs|sdks]`。

- [ ] **Step 1: 写失败测试**

- `rewritePathDeps` 对含 `path = "../common-error"` 的片段输出 `version = "1.7.0"`，注释与 `name` 行不变
- 拓扑顺序常量：`LIBS_ORDER` 中 `common-error` 在 `common` 之前；`SDKS_ORDER[0] === 'digua-http-client'`
- **回滚**：注入 `exec`，在第 3 个 crate 抛错 → 断言 `fs.writeFileSync` 收到的第二次写入内容等于原始文本（即已复原），且错误为 `ExternalError`
- `dryRun` 时 `exec` 零调用

- [ ] **Step 2: 运行测试确认失败 → Step 3: 实现 → Step 4: 测试通过**

Run: `node --test packages/cli/test/rust-publish.test.js`
Expected: FAIL 后 PASS

- [ ] **Step 5: 提交**

```bash
git add packages/cli
git commit -m "feat(cli): absorb rust crate publishing with restore-on-failure guarantee"
```

---

### Task 9: `coder release publish npm`

**Files:**
- Create: `packages/cli/lib/npm-packages.js`（自 `scripts/check-packages-publish.js` 移植）
- Create: `packages/cli/test/npm-packages.test.js`
- Modify: `packages/cli/commands/release.js`

**Interfaces:**
- Produces:
  - `PACKAGES = [{ dir: 'packages/contracts', name: '@digua/contracts' }, { dir: 'packages/http', name: '@digua/http' }]`
  - `checkPackages({ root, nextVersion, registryUrl = 'http://npm.kudi.repos/', httpGet, exec }) -> { ok, failures, notes }`：校验项与现有脚本一致（包存在、`version` 可被覆盖、registry 可达、dist 构建产物存在）
  - `publishPackages({ root, version, dryRun, exec, check = checkPackages }) -> { published }`：`pnpm run packages:build` → `check(...)` → 每个包 `npm version <v> --no-git-tag-version` → `npm publish --prefix <dir>`
- 命令：`coder release publish npm --version X [--dry-run]`。

- [ ] **Step 1: 写失败测试**

- registry 不可达（`httpGet` 抛错）→ `ok:false`、`failures` 含 registry 项
- `nextVersion: 'v1.7.0'` 归一化为 `1.7.0`
- `publishPackages` 的 `exec` 序列：`packages:build` 在最前、两个 `npm publish` 在最后，且都发生在 `checkPackages` 通过之后（注入 `check` 失败时序列只剩 build）

- [ ] **Step 2: 运行测试确认失败 → Step 3: 实现 → Step 4: 测试通过**

Run: `node --test packages/cli/test/npm-packages.test.js`

- [ ] **Step 5: 提交**

```bash
git add packages/cli
git commit -m "feat(cli): absorb npm package publishing"
```

---

### Task 10: CI 切换、旧脚本删除与入口瘦身

**Files:**
- Modify: `.gitlab-ci.yml`（`sdk_publish`、`rust_publish`、`build_images:staging`、`build_images:tagged`、`publish_packages` 五个 job 的命令；`release:dist` 本批不动）
- Modify: `scripts/ci-local-run.js`（这五个 job 的本地映射命令同步改为 `coder`）
- Delete: `scripts/build-docker.js`、`scripts/publish-libs.js`、`scripts/publish-rust-sdks.js`、`scripts/publish-maven-release.js`、`scripts/publish-maven-snapshot.js`、`scripts/check-packages-publish.js`、`scripts/download-swagger-ui.js`、`scripts/lib/maven-publish.js`
- Modify: `package.json`（`docker:build*`、`assets:*`、`sdk:publish:*`、`packages:*`、`test:all`）
- Modify: `docs-site/guides/java-sdk-publishing.md`、`docs-site/guides/rust-sdk-publishing.md`、`docs-site/runbooks/release-tag-pipeline.md`、根 `AGENTS.md`（命令名替换）

**Interfaces:**
- 消费：Task 3–9 的全部命令。
- 完成后 `pnpm run` 中发布相关条目只剩 `coder ...` 转发；`scripts/` 减少 8 个文件。

- [ ] **Step 1: 切换 CI**

逐 job 替换命令（保留各自的 `before_script`、`rules`、`environment`）：
- `sdk_publish`：`node scripts/publish-maven-release.js --version "${VERSION}"` → `node packages/cli/bin/coder.js release publish maven --version "${VERSION}"`；同时**删除 `npm ci` 与 `apt-get install nodejs npm` 两行**（CLI 零依赖，maven 镜像只需 node 二进制；若镜像确认无 node 则保留 apt 安装 node 但去掉 `npm ci`）与随之失效的注释。
- `rust_publish`：两条 `node scripts/publish-*.js` → `node packages/cli/bin/coder.js release publish rust "${VERSION}" --registry kellnr`。
- `build_images:staging` / `build_images:tagged`：`node scripts/download-swagger-ui.js` → `node packages/cli/bin/coder.js assets download`；`node scripts/build-docker.js …` → `node packages/cli/bin/coder.js build images …`。
- `publish_packages`：`node scripts/check-packages-publish.js` + `npm publish` 链 → `node packages/cli/bin/coder.js release publish npm --version "$VERSION"`。

- [ ] **Step 2: 更新 ci-local 映射并跑一次本地预演**

改 `scripts/ci-local-run.js` 的 `JOBS` 中五个对应条目的 `cmd`；Run: `node scripts/ci-local-run.js --list`
Expected: 五个 job 显示新的 `coder` 命令

- [ ] **Step 3: 删除旧脚本与根 script 瘦身**

删上列 8 个文件；`package.json` 中：
- `docker:build`、`docker:build:<svc>` ×9 → `node packages/cli/bin/coder.js build images [<svc>]`
- `docker:build:clean` → `node packages/cli/bin/coder.js build images prune`
- `assets:download` / `:force` / `:check` → `coder assets download [--force|--check]`
- `sdk:publish:release|snapshot|libs|rust` → `coder release publish maven --version`（release；snapshot 用 `--kind snapshot`）/ `coder release publish rust`
- `packages:check` / `packages:publish` → `coder release publish npm`
- `test:all` 追加 `pnpm run test:cli`

- [ ] **Step 4: 全量验证**

Run: `node --test packages/cli/test/ && node packages/cli/bin/coder.js doctor --json && node scripts/run-tests-host.js --no-db`
Expected: 测试全绿；`doctor` 报 `context: 'monorepo'`；Rust 单测不受影响

Run: `grep -rn "scripts/build-docker\|scripts/publish-libs\|scripts/publish-rust-sdks\|scripts/publish-maven\|scripts/check-packages-publish\|scripts/download-swagger-ui" --include='*.js' --include='*.yml' --include='*.md' --include='*.json' . | grep -v node_modules | grep -v 'release/dist-template/release/'`
Expected: 无输出（`release/dist-template/scripts/*` 属批次三，不在本批范围）

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "refactor(ci)!: route publish pipeline through coder CLI and drop legacy scripts"
```

---

## 批次一的完成判据

1. `node packages/cli/bin/coder.js release status --json` 在当前仓库输出三处账本对账结果与退出码 3（Makefile/CHANGELOG 仍停在 1.0.0，属预期漂移）。
2. `coder build images`、`coder release publish {maven,rust,npm}`、`coder assets download` 的参数面与旧脚本等价，且全部支持 `--json` 与 `--dry-run`。
3. `scripts/` 下 8 个被收编文件删除，CI 与 ci-local 全部改调 `coder`。
4. `node --test packages/cli/test/` 全绿；`test:all` 纳入 `test:cli`。
5. 计划内所有 Review Focus 项都有对应测试并通过。
