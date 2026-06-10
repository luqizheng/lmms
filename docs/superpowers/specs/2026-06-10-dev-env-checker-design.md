# 开发环境检查脚本 — 设计

- 日期：2026-06-10
- 状态：草案
- 范围：`scripts/check-env.mjs` + AI 启动触发规则

## 目标

首次从 git 仓库拉取代码并让 LLM 编码时，自动验证本地开发环境是否齐备。任何缺失项必须显式提示，避免在缺工具链的情况下产生误导性失败（典型如：Rust 缺 MSVC v143 → `fiat_25519` 链接错误；Docker 未启动 → compose 报 daemon 错）。

## 前置条件

- 脚本运行本身依赖 Node 18+（使用 `node:` 前缀内置模块）。这是检查 `Node` 18+ 项的鸡生蛋前提，假定开发者机器上 Node 18+ 是已就绪的（与本仓库 `frontend/package.json` 的 Node 要求一致）。
- 适用平台：Windows 10+、macOS 12+、Ubuntu 20.04+。其他平台不保证。

## 范围

### 检查项

| 工具 | 检测命令 | 最低版本 | 平台 |
|------|---------|---------|------|
| Node | `node -v` | 18.0.0 | 全平台 |
| Docker | `docker --version` | 20.10.0 | 全平台 |
| PowerShell | `pwsh -v` (Linux/macOS) / `$PSVersionTable.PSVersion.Major` (Windows) | 5.1 | 全平台 |
| Git | `git --version` | 2.30.0 | 全平台 |
| Rust | `rustc --version` | 1.75.0 | 全平台 |
| Cargo | `cargo --version` | 1.75.0 | 全平台 |
| MSVC v143 | `vswhere.exe -products * -property installationPath` + 工作负载扫描 | required | Windows only |

### 不检查

- MySQL / Redis / Consul 连通性（属 docker compose 启动后范畴）
- Node 包安装状态（`pnpm install` 单独负责）
- 网络可达性（`crates.io` / `registry.npmjs.org`）

## 文件结构

```
scripts/
  check-env.mjs          # 主入口，零依赖
  lib/
    EnvChecker.mjs        # 检查器聚合
    VersionParser.mjs     # 版本字符串解析与比较
.trae/rules/
  project_rules.md        # 追加会话启动规则
package.json              # 新增 "check:env" script
```

## 行为

### 主流程

1. 检测平台（`process.platform`）
2. 遍历检查项，按下表命令探测
3. 解析输出，提取语义化版本号
4. 与最低版本比较，输出表格
5. 汇总失败项，输出提示与退出码

### 版本解析

`VersionParser` 接受任意字符串，返回 `{ major, minor, patch }`：

- `20.11.0` → `20.11.0`
- `Docker version 24.0.7, build ...` → `24.0.7`
- `rustc 1.78.0 (9b00956e3 2024-04-29)` → `1.78.0`
- `powershell 7.4.1` / `5.1.19041.3031` → 通用处理
- 无法解析时该项判 FAIL，原因 `unparseable`

### 比较

`compare(detected, required)`：

- 数字三元组逐位比较
- `detected < required` → FAIL
- `detected >= required` → OK
- required 为 `required`（非数字）时只判存在性

### MSVC v143 检测（Windows only）

1. 用 `where vswhere.exe` 定位 `vswhere`（一般位于 `C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe`）
2. 执行 `vswhere.exe -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`
3. 退出码 0 且输出非空 → OK；否则 FAIL 并提示安装 VS Build Tools 2022 + "使用 C++ 的桌面开发" 工作负载

非 Windows 平台直接跳过此检查，标 `SKIP`。

### 输出

控制台表格，列：`Tool`、`Required`、`Detected`、`Status`。对齐方式：固定列宽。

```
Tool          Required    Detected           Status
-----------   ---------   ----------------   ------
Node          >= 18.0     20.11.0            OK
Docker        >= 20.10    24.0.7             OK
PowerShell    >= 5.1      5.1.19041          OK
Git           >= 2.30     2.43.0             OK
Rust          >= 1.75     not found          FAIL
Cargo         >= 1.75     not found          FAIL
MSVC v143     required    not found          FAIL

Result: 3 check(s) failed.
- Rust:  install via https://rustup.rs
- Cargo: install via https://rustup.rs
- MSVC v143: install "Desktop development with C++" from VS Build Tools 2022
```

颜色：

- `OK` 绿色
- `FAIL` 红色，整行加粗
- `SKIP` 灰色

降级：检测 `NO_COLOR` 环境变量或 `!process.stdout.isTTY` 时，去除 ANSI 序列。

### 退出码

- 全 OK 或仅 SKIP：`process.exit(0)`
- 任意 FAIL：`process.exit(1)`

### `--json` 选项

输出 JSON 数组到 stdout，结构：

```json
[
  {"tool":"node","required":">=18.0","detected":"20.11.0","status":"ok"},
  {"tool":"rust","required":">=1.75","detected":null,"status":"fail","hint":"install via https://rustup.rs"}
]
```

退出码语义不变。

## 错误处理

- 命令超时 5s（`child_process.exec` + `timeout: 5000`）：该项 FAIL，标 `timeout`
- 子进程非零退出且无输出：判不存在，FAIL
- 抛错：捕获后该项 FAIL，原样打印错误到 stderr
- 任何脚本自身异常不吞掉，stderr 输出并退出码 2

## 测试

- `scripts/lib/VersionParser.mjs` 覆盖常见版本字符串解析
- `scripts/lib/EnvChecker.mjs` 的核心比较逻辑使用 mock 子进程结果做单元验证
- 端到端手动验证：临时删除某工具（PATH 屏蔽），确认脚本正确报 FAIL

测试入口建议：`scripts/lib/__tests__/`，但本仓库无 Node 测试框架约定，故本期仅以手工验证 + 解析器自包含断言演示；后续如引入 `node:test` 再补 CI。

## AI 触发规则

在 `.trae/rules/project_rules.md` 顶部追加：

```markdown
## 会话启动

- AI 第一次接收本仓库编码任务时，必须先执行：
  `node scripts/check-env.mjs`
- 脚本退出码非 0 时，输出缺失项清单并终止后续动作，提示用户补齐环境后重试。
- 已通过检查的会话内不必重复跑，除非用户主动要求或环境发生变化。
```

## 命令入口

`package.json`：

```json
"scripts": {
  "check:env": "node scripts/check-env.mjs"
}
```

## 风险与约束

- 零依赖：禁止引入 chalk / cli-table 等，避免 CI 与本地都要 `pnpm install`
- Node 18+：脚本使用 `node:` 前缀内置模块（`node:child_process`、`node:process`），需 Node 18+
- Windows ANSI：Win10 以下默认 cmd 不支持 ANSI；脚本检测到 Windows 且非 `WT_SESSION` 时降级
- vswhere 路径因 VS 版本而异，统一通过 `where vswhere.exe` 定位，不硬编码
