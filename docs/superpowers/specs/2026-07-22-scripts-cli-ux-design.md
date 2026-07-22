# scripts/ CLI 用户体验优化 —— 引入 chalk / ora / prompts

**Date**: 2026-07-22
**Branch**: `feat/scripts-cli-ux`
**Status**: Design

## 1. 目标与非目标

### 目标
把 `scripts/` 下 40+ 个 Node 脚本的 CLI 体验统一到业界标准：
- **颜色输出**：全量迁 `chalk` 原生 API。删除 `scripts/lib/utils.js` 里的 `paint()` / `log.*` / `COLORS` 自制封装。
- **交互输入**：用 `prompts` 替换 `readline` 手搓 Y/N 与 question。
- **进度显示**：给"静默长时任务"套 `ora` spinner（下载、docker 启动、HTTP 轮询等）。

### 非目标
- 不改脚本的**行为语义** —— 只改显示层。`--non-interactive` / `--yes` / `--force` 等既有 flag 全部保持向后兼容。
- 不动 `dump-openapi-specs.sh` / `publish-sdks.sh` 等 shell 脚本。
- 不改 `stdio: 'inherit'` 类脚本的子进程输出方式（build-cargo / contracts-gen / dev-setup / run-e2e-local 等）—— 只把它们的**周边日志**换成 chalk。子进程本身继续 inherit stdout/stderr，让 cargo/docker 输出照旧滚动。
- 不为 CI-only 脚本（check-service-standard / check-spec / check-db-naming 等）加交互；它们只做颜色升级。

## 2. 依赖选型

| 包 | 版本 | 模块系统 | 理由 |
|---|---|---|---|
| `chalk` | `^4.1.2` | CJS（`require()` 可用） | v5 是 ESM-only。全仓 CJS，改 mjs 成本大。 |
| `ora` | `^5.4.1` | CJS | v6+ ESM-only。v5 API 完整：`start / succeed / fail / warn / info / stop`。 |
| `prompts` | `^2.4.2` | CJS | 唯一稳定 CJS 交互库。API 简洁。 |

装到 `devDependencies`：

```jsonc
"devDependencies": {
  "chalk": "^4.1.2",
  "ora": "^5.4.1",
  "prompts": "^2.4.2",
  // ... 原有
}
```

## 3. 改造范围

### 3.1 摸底数据（grep 结果）

| 项 | 数字 |
|---|---|
| `scripts/*.js` 总数 | 40+ |
| 引用 `scripts/lib/utils.js` 且用其颜色 API 的文件 | 3（setup-wizard / lib/maven-publish / base-images-sync） |
| `log.*` 调用总数 | 136（83 + 41 + 12） |
| 直接用 `\x1b[` ANSI 拼接的文件 | 6（check-dev-env / check-migration-state / dev-setup / build-docker / download-swagger-ui / e2e-migration-state） |
| 用 readline 交互的脚本 | 2（setup-wizard、build-docker） |
| 适合套 ora 的脚本 | ~6（download-swagger-ui / run-tests-docker / release-push / check-dev-env / e2e-event-flow / e2e-trace-id-flow） |
| `stdio: 'inherit'` 类（不套 spinner） | 7 |

### 3.2 分层策略

**Layer 1 — 基础库改造**（1 个文件）
- `scripts/lib/utils.js`：
  - **删除** `paint()` / `log` 对象 / `COLORS` 常量的导出。
  - **保留** `ROOT` / `reportsDir` / `runDir`（非颜色 API，仍被下游使用）。
- 该文件占位符 `paint` 保留？不。删除后所有引用它的地方都会立刻 crash，正好逼迫 Layer 3-5 完成迁移，避免留兼容层。

**Layer 2 — 新增共用模块**（2 个新文件）
- `scripts/lib/ui.js`：导出统一 UI helpers。
  ```js
  const chalk = require('chalk');
  const ora = require('ora');
  module.exports = {
    chalk,
    // 语义化日志：4 种带图标的输出
    success: (msg) => console.log(chalk.green('✓'), msg),
    warn:    (msg) => console.log(chalk.yellow('⚠'), msg),
    error:   (msg) => console.error(chalk.red('✗'), msg),
    info:    (msg) => console.log(chalk.cyan('ℹ'), msg),
    // 段落标题
    section: (title) => console.log('\n' + chalk.bold.cyan(`▸ ${title}`)),
    banner:  (title) => console.log('\n' + chalk.bold.bgBlue.white(` ${title} `) + '\n'),
    // spinner 工厂：非 TTY / CI 时降级为普通日志
    spinner: (text) => {
      if (!process.stdout.isTTY || process.env.CI) {
        console.log(chalk.gray('…'), text);
        return {
          start() { return this; },
          succeed(msg) { console.log(chalk.green('✓'), msg || text); return this; },
          fail(msg)    { console.error(chalk.red('✗'), msg || text); return this; },
          warn(msg)    { console.log(chalk.yellow('⚠'), msg || text); return this; },
          info(msg)    { console.log(chalk.cyan('ℹ'), msg || text); return this; },
          stop()       { return this; },
          text:  '',
        };
      }
      return ora({ text, spinner: 'dots' });
    },
  };
  ```
- `scripts/lib/prompt.js`：包一层 `prompts`。
  ```js
  const prompts = require('prompts');
  const chalk = require('chalk');

  const onCancel = () => { console.error(chalk.yellow('\n✗ 操作被取消')); process.exit(130); };

  async function ask(config, { nonInteractive = false } = {}) {
    if (nonInteractive) {
      // 非交互模式下短路：返回 initial
      return { [config.name]: config.initial };
    }
    return prompts(config, { onCancel });
  }

  module.exports = {
    ask,
    async input(message, initial, opts = {}) {
      const r = await ask({ type: 'text', name: 'v', message, initial }, opts);
      return r.v;
    },
    async confirm(message, initial = false, opts = {}) {
      const r = await ask({ type: 'confirm', name: 'v', message, initial }, opts);
      return r.v;
    },
    async select(message, choices, initial = 0, opts = {}) {
      const r = await ask({ type: 'select', name: 'v', message, choices, initial }, opts);
      return r.v;
    },
    async multiselect(message, choices, opts = {}) {
      const r = await ask({ type: 'multiselect', name: 'v', message, choices, hint: '空格选择, 回车确认' }, opts);
      return r.v;
    },
    async password(message, opts = {}) {
      const r = await ask({ type: 'password', name: 'v', message }, opts);
      return r.v;
    },
  };
  ```

**Layer 3 — 交互脚本改造**（2 个文件）
- `scripts/setup-wizard.js`（878 行，83 处 log 调用，1 个 readline 使用点）：readline → prompts；`log.*` → `chalk.*` 或 `ui.success/warn/error/info`。保留 `--non-interactive/--defaults/--show/--dry-run` 语义。
- `scripts/build-docker.js`（1 个 readline 使用点 + 自制 COLORS 对象）：readline callback → prompts；自制 COLORS 对象删除，改用 chalk。保留现有 `--yes/--force/--no-cache` 等 flag。

**Layer 4 — Spinner 加装**（6 个文件）
- `download-swagger-ui.js`：HTTPS 下载套 ora，`spinner.text` 显示 `[已下载 XX MB / 共 YY MB]`。
- `run-tests-docker.js`：docker container 检查、mysql 启动、镜像拉取阶段套 ora。
- `release-push.js`：git 状态检查、rev-parse 阶段套 ora（push 本身用 inherit 不套）。
- `check-dev-env.js`：环境检查逐项套 ora（Docker / Node / pnpm / Rust ...）。
- `e2e-event-flow.js` / `e2e-trace-id-flow.js`：HTTP 轮询等待阶段套 ora。

**Layer 5 — 纯颜色迁移**（其余所有脚本）
- **依赖 utils.js 的下游**：`scripts/lib/maven-publish.js`（41 处）、`scripts/base-images-sync.js`（12 处）—— 全部把 `log.red(t)` / `log.plain(t)` 等改成 `chalk.red(t)` / `console.log(t)`。
- **直接用 `\x1b[` 的脚本**：`check-migration-state.js`、`e2e-migration-state.js`、`check-dev-env.js`（部分）、`dev-setup.js`（COLORS 对象）、`build-docker.js`（COLORS 对象）、`download-swagger-ui.js`（logOk/logErr/logWarn）—— 全部改用 chalk。
- **其他 check-*.js**：只有一小部分带颜色，随手改。

## 4. setup-wizard 改造细节

现在的 prompt 位置（scripts/setup-wizard.js:443, :458, :486）：
```js
const readline = require('node:readline/promises');
const rl = readline.createInterface({ input: stdin, output: stdout });
const answer = await rl.question(`${question}${suffix}: `);
```

改造后：
```js
const { input, confirm, select } = require('./lib/prompt');
const answer = await input(question, defaultValue, { nonInteractive: isNonInteractive });
```

对应关系：
- 单行文本输入 → `prompts({type:'text'})`
- Y/N 确认 → `prompts({type:'confirm'})`
- 从枚举里选 → `prompts({type:'select'})`
- Password/token → `prompts({type:'password'})`

`--non-interactive` 保持不变：`lib/prompt.js` 的 `ask()` 检测 opts.nonInteractive 时短路返回 initial。caller 层已经在管这个 flag，语义完全一致。

**关键：不改问题顺序、不改问题文案、不改默认值** —— 只换渲染引擎。行为一致。

## 5. `chalk` API 迁移映射

### 5.1 utils.js 老 API → chalk 新 API

| 老 API | 新 API |
|---|---|
| `log.red(t)` | `console.error(chalk.red(t))`（错误场景）或 `console.log(chalk.red(t))` |
| `log.green(t)` | `console.log(chalk.green(t))` 或 `ui.success(t)` |
| `log.yellow(t)` | `console.log(chalk.yellow(t))` 或 `ui.warn(t)` |
| `log.blue(t)` | `console.log(chalk.blue(t))` |
| `log.magenta(t)` | `console.log(chalk.magenta(t))` |
| `log.cyan(t)` | `console.log(chalk.cyan(t))` 或 `ui.info(t)` |
| `log.gray(t)` | `console.log(chalk.gray(t))` |
| `log.bold(t)` | `console.log(chalk.bold(t))` |
| `log.plain(t)` | `console.log(t)` |
| `paint('red', t)` | `chalk.red(t)` |
| `COLORS.red` 拼接 | 直接删（内部实现） |

### 5.2 直接 ANSI 迁移

| 老写法 | 新写法 |
|---|---|
| `` `\x1b[32m${msg}\x1b[0m` `` | `chalk.green(msg)` |
| `` `\x1b[31m${msg}\x1b[0m` `` | `chalk.red(msg)` |
| `` `\x1b[33m${msg}\x1b[0m` `` | `chalk.yellow(msg)` |
| 自制 `const COLORS = {...}` 对象 | 直接删，全部用 chalk |

## 6. `ora` 使用规范

**只在"静默长时任务"上用**。触发条件（AND）：
1. 单次持续 ≥ 1s
2. 子进程 stdout 用 `pipe`（否则会撞掉 spinner 帧）
3. 无 stdin 交互穿插

**不套 spinner 的场景**：
- `stdio: 'inherit'` 的 spawn（cargo build、docker build、pnpm test 等）—— 让子进程直接刷屏。
- 循环打印多行日志的场景。
- CI 环境（`process.env.CI` 存在时）—— `ui.spinner()` helper 自动降级为普通日志（见 §3.2 Layer 2 代码）。

## 7. 落地顺序

**一个 commit 全做完** —— 遵循项目规则"一个任务一个 commit"。

**Commit** (`feat(scripts): migrate CLI UX to chalk/ora/prompts`) 包含：
1. `package.json` 加 3 个 devDep + `pnpm install`
2. 新增 `scripts/lib/ui.js` + `scripts/lib/prompt.js`
3. 改 `scripts/lib/utils.js`：删除颜色 API，保留 ROOT/reportsDir/runDir
4. Layer 3 交互脚本（setup-wizard + build-docker）
5. Layer 4 spinner 加装（6 个脚本）
6. Layer 5 纯颜色迁移（maven-publish + base-images-sync + 6 个直接 ANSI 用户）

Diff 预计 400-700 行（大部分是 sed 式的一对一替换）。

## 8. 验证

- `node scripts/setup-wizard.js --show` → 输出配置，无交互报错
- `node scripts/setup-wizard.js --non-interactive --defaults` → 全跑默认值，退出码 0
- `node scripts/setup-wizard.js`（TTY）→ 手动跑一遍所有 prompt
- `node scripts/build-docker.js api-gateway --yes` → 非交互构建
- `node scripts/download-swagger-ui.js --force` → 观察 spinner 是否显示进度
- `CI=1 node scripts/download-swagger-ui.js --force` → 观察 spinner 是否降级为普通日志
- `node scripts/check-service-standard.js` → 颜色输出正常
- `node scripts/base-images-sync.js --check` → 颜色输出正常
- `NO_COLOR=1 node scripts/base-images-sync.js` → 无颜色输出（chalk 自动尊重）
- CI 门禁三件套 `check-service-standard / check-spec / contracts-gen --check` 全过

## 9. 风险

| 风险 | 缓解 |
|---|---|
| chalk v4 CJS 兼容问题 | v4 已确认是 CJS，`npm view chalk@4.1.2` 无 `"type":"module"`。 |
| ora spinner 在非 TTY / CI 环境撞行 | `lib/ui.js` 的 `spinner()` 检测 `!process.stdout.isTTY \|\| process.env.CI` 时降级为普通日志。 |
| prompts Ctrl+C 泄漏 | `lib/prompt.js` 统一注入 `onCancel` → 退出码 130。 |
| CI 门禁颜色输出问题 | chalk 自动尊重 `NO_COLOR` / `FORCE_COLOR` 环境变量。 |
| 遗漏某处 log/paint/COLORS 调用 | 删除 utils.js 导出后，遗漏点会立刻 crash，反而暴露问题。commit 前跑 §8 验证清单兜底。 |
| readme / docs 里若引用了脚本文案 | 文案保持不变，无影响。 |
| Windows PowerShell 兼容 | chalk 与 ora 都原生支持 Windows；prompts 也支持。 |

## 10. 不改的东西

- shell 脚本（`dump-openapi-specs.sh` / `publish-sdks.sh`）
- `git-hooks/` 目录下脚本
- `scripts/prompts/` 空目录（保留，可能是未来占位）
- `scripts/releases/` 目录
- `scripts/lib/Base-Images-Scanner.js` / `Base-Images-Writer.js`（若不用 log/paint，则不动）
