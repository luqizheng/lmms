---
name: "coder-lib-upgrade"
description: "Upgrades common-* libraries (common, common-http, common-crypto, etc.) across the workspace. Invoke when user asks to upgrade libs, update dependencies, or sync common library versions."
---

# coder-lib-upgrade

## 何时用

用户说「升级通用库」「更新依赖」「同步common库版本」时调用本skill。

## 前置

- 运行环境：WSL / macOS / Linux（Windows 必须在 WSL 内执行）
- 工作目录：项目根目录
- Rust 工具链已安装

## 步骤

### 1. 检查当前库版本

```bash
cargo tree -p common --invert
cargo tree -p common-http --invert
cargo tree -p common-crypto --invert
```

### 2. 升级库依赖

```bash
# 升级单个库
cargo upgrade -p <lib-name>

# 升级全部common-*库
cargo upgrade -p common -p common-http -p common-crypto -p common-db -p common-log -p common-error
```

### 3. 更新Cargo.toml

编辑各服务的 `Cargo.toml`，更新 `common-*` 依赖版本

### 4. 编译验证

```bash
cargo build --workspace
```

### 5. 运行测试

```bash
node scripts/test-orchestrator.js test-all
```

## 常用场景

```bash
# 升级到最新稳定版
cargo update

# 检查过时依赖
cargo outdated

# 仅检查不升级
cargo check -p common -p common-http
```

## 故障排查

- 编译失败：检查版本兼容性
- 链接错误：确认 `aws_lc_rs` crypto provider
