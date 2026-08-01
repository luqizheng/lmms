# Rust SDK 发布指南

> 适用：把 `sdk/rust/` 下的 7 个 crate 发布到 kellnr（内网 registry）。
> 关联：[`sdk/rust/README.md`](../../sdk/rust/README.md)（SDK 用法）、[`scripts/publish-rust-sdks.js`](../../scripts/publish-rust-sdks.js)（发布脚本，Node.js，跨平台）。
> 维护：本文档随发布脚本和 registry 政策变化同步更新。

---

## 1. SDK crate 总览

```
sdk/rust/
├── digua-http-client/      # 目录名（crate name = digua_http_client，先发布）
├── audit-service-sdk/      # 6 个 service SDK 都依赖 digua_http_client（path 依赖）
├── config-service-sdk/     # （path 依赖在本地开发时；发布时切 version）
├── file-service-sdk/
├── iam-service-sdk/
├── notification-service-sdk/
└── token-service-sdk/
```

每个 crate 独立打包为 `*.tar.gz` 上传到 [crates.io](https://crates.io)，最终用户通过 `cargo add <name>` 或 `Cargo.toml` 依赖引入。

| crate | 版本 | registry |
| --- | --- | --- |
| `digua_http_client` | 1.0.0 | kellnr |
| `digua_iam_sdk` | 1.0.0 | kellnr |
| `digua_token_sdk` | 1.0.0 | kellnr |
| `digua_audit_sdk` | 1.0.0 | kellnr |
| `digua_file_sdk` | 1.0.0 | kellnr |
| `digua_notification_sdk` | 1.0.0 | kellnr |
| `digua_config_sdk` | 1.0.0 | kellnr |

---

## 2. 前置条件

发布之前请确认：

| # | 条件 | 怎么验证 |
| - | --- | --- |
| 1 | kellnr registry 已配置 + token | `~/.cargo/credentials.toml` 有 `[registries.kellnr]` section（`scripts/setup-wizard.js` 自动写） |
| 2 | 本机 `cargo` 1.70+ | `cargo --version` |
| 3 | 项目级 `CARGO_TARGET_DIR` 可写 | 仓库根 `target/` 默认是 docker volume 的（root-owned），必须用 `CARGO_TARGET_DIR=/tmp/sdk-publish` 绕开 |
| 4 | `.cargo/config.toml` 配置正确 | 必须有 `[registries.kellnr]` 的 index URL |

> ⚠️ **Cargo 必须能访问 crates.io 真正的 index**。如果项目级 `.cargo/config.toml` 把 `[source.crates-io]` replace-with 到了 Panamax 之类的只读 mirror，**publish 时会失败**（mirror 不接受上传）。要么发布前临时禁掉 mirror（§5.2），要么 publish 时绕开 mirror。

---

## 3. 一次性配置

### 3.1 配置 API token

用环境变量最安全（不进 shell history / git）：

```bash
export CARGO_REGISTRY_TOKEN=<your-token-here>
```

或者写入 `~/.cargo/credentials.toml`：

```toml
# ~/.cargo/credentials.toml
[registries.crates-io]
token = "<your-token-here>"
```

权限设为 `chmod 600`。

> ⚠️ 不要把 token commit 到 git。`.gitignore` 应该排除 `credentials.toml`。

### 3.2 切换到 sparse index（推荐）

如果项目用了只读 git-protocol mirror，把 `[source.crates-io].replace-with` 切到 sparse（`panamax-sparse`）：

```toml
# .cargo/config.toml
[source.panamax-sparse]
registry = "sparse+http://192.168.2.12:16444/index/"

[source.crates-io]
replace-with = "panamax-sparse"
```

sparse index 体积小、速度快。git-protocol mirror（`panamax` / `panamax-git`）只在你坚持要用 git index 时才配。

### 3.3 crate 必填字段

每个 `sdk/rust/*/Cargo.toml` 必须有 `description` 和 `license`，否则 `cargo publish` 拒绝：

```toml
[package]
name = "..."
version = "0.1.0"
edition.workspace = true
description = "..."     # ← 必填
license = "Apache-2.0"  # ← 必填
```

建议再加 `repository` / `homepage` 抑制 warning：

```toml
repository = "https://github.com/your-org/your-repo"
homepage = "https://github.com/your-org/your-repo"
```

脚本会自动校验缺字段并报错退出（见 §5.1）。

---

## 4. 发布顺序与依赖关系

```
digua_http_client   ←  基础库，先发
       ↑
       ├── digua_audit_sdk
       ├── digua_config_sdk
       ├── digua_file_sdk
       ├── digua_iam_sdk
       ├── digua_notification_sdk
       └── digua_token_sdk      ← 6 个 service SDK，每个独立，互不依赖
```

发布顺序：先发 `digua_http_client`，等 crates.io 索引（约 30 秒）后，再发 6 个 service SDK。6 个 service SDK 之间互不依赖，可以串行或并行。

> ⚠️ 6 个 SDK 本地 Cargo.toml 用 `path = "../digua_http_client"`（workspace path 依赖），`cargo publish` 拒绝 path 依赖。脚本会自动把它们临时改为 `version = "<x.y.z>"`，发完后恢复（见 §5.1）。

---

## 5. 发布流程

### 5.1 用发布脚本（推荐）

```bash
# 从仓库根 —— VERSION 是必填位置参数。跨平台（Linux / macOS / Windows）
node scripts/publish-rust-sdks.js <VERSION> [--dry-run] [--registry <name>]
```

**参数：**

| 参数 | 说明 |
| --- | --- |
| `<VERSION>`（位置参数，**必填**） | 统一版本号（例如 `1.0.0`）；脚本检查所有 crate 版本是否一致 |
| `--dry-run` | 只验证，不上传 + 不修改 Cargo.toml |
| `--registry <name>` | 上传到指定 registry（默认 `kellnr`） |
| `--help` | 打印完整用法 |

**脚本做的事：**

1. **前置检查**：每个 crate 必须有 `description` + `license`。`<VERSION>` 必填。
2. **版本检查**：所有 crate 的 `version` 必须与 `<VERSION>` 一致。
3. **发 digua_http_client**：基础库，直接 `cargo publish`。
4. **替换 path 依赖**：把 6 个 service SDK 的 `path = "../digua-http-client"` 临时改为 `version = "<ver>"` + `registry = "kellnr"`，备份到 `.bak`。
5. **逐个发 service SDK**：编译 → 打包 → 上传。
6. **恢复 path 依赖**：mv `.bak` 还原。
7. **汇总**：成功 / 失败列表。

**典型执行：**

```bash
# 第一次发布 / 验证流程
node scripts/publish-rust-sdks.js 1.0.0 --dry-run

# 确认无误后真发布
CARGO_TARGET_DIR=/tmp/sdk-publish \
  node scripts/publish-rust-sdks.js 1.0.0

# 或通过 pnpm
pnpm run sdk:publish:rust -- 1.0.0 --dry-run
CARGO_TARGET_DIR=/tmp/sdk-publish pnpm run sdk:publish:rust -- 1.0.0
```

> Windows PowerShell：`$env:CARGO_TARGET_DIR="$env:TEMP\sdk-publish"; node scripts/publish-rust-sdks.js 1.0.0`

---

### 5.2 使用 kellnr registry

脚本默认发布到 `kellnr`（内网 registry），无需额外配置。若需发布到其他 registry：

```bash
node scripts/publish-rust-sdks.js 1.0.0 --registry my-registry
```

前提是 `~/.cargo/credentials.toml` 中已配置对应 registry 的 token：

```toml
[registries.kellnr]
token = "xxx"

[registries.my-registry]
token = "yyy"
```

---

## 6. 发布后验证

```bash
# 拉一个临时目录试装
mkdir /tmp/sdk-verify && cd /tmp/sdk-verify
cargo init --name verify-app
cargo add digua_iam_sdk digua_http_client --registry kellnr
cargo build           # 期望：从 kellnr 拉 digua_http_client 和 digua_iam_sdk
cargo tree | head      # 期望：能看到 digua_http_client v1.0.0 是从 kellnr 拉的
```

或在文档站浏览：[`./rust-sdk`](./rust-sdk) §「SDK 现状」给出 `cargo add` 示例。

---

## 7. crates.io 速率限制

crates.io 对**新账号**和**高频账号**有 rate limit：

| 限制 | 阈值 | 触发后报错 |
| --- | --- | --- |
| 24h 内发布 crate 数 | 取决于账号年龄 + 历史 | `429 Too Many Requests: You have published too many new crates in a short period of time. Please try again after <时间戳>` |

**应对：**

- **顺序发，不要并行**：脚本默认串行发；如果你想并行，限流后**退到串行**。
- **看错误里的时间戳**：crates.io 会告诉你具体几分钟后可以重试。`date -u` 对照一下。
- **被限流的 crate 重试即可**：crates.io 接受重试，已成功的 crate 不会重复发布。

第一次发 7 个 crate 时，可能在第 5~6 个遇到 429。等错误里说的时间过后，重跑脚本（脚本是幂等的：已发的跳过，未发的继续）。

---

## 8. 版本号管理与后续发布

### 8.1 何时 bump 版本号

| 改动类型 | 版本号变化 |
| --- | --- |
| 修复 bug、不破坏 API | patch：`0.1.0 → 0.1.1` |
| 新增方法（向后兼容） | minor：`0.1.0 → 0.2.0` |
| 改动签名 / 删方法（破坏兼容） | major：`0.1.0 → 1.0.0` |
| 0.x 阶段任意不向后兼容改动 | 直接 minor+1：`0.1.0 → 0.2.0` |

### 8.2 一致性约束

`digua_http_client` 和所有 service SDK **必须使用同一个版本号**（脚本默认行为）。原因：

- service SDK 依赖 `digua_http_client = { version = "0.1.0" }`（发布时脚本自动注入）
- 如果 digua_http_client 升到 `0.2.0` 而 service SDK 还在 `0.1.0`，会引入两个版本共存问题

发布流程：

1. 确保 7 个 crate 的 `version` 与预期 `<VERSION>` 一致
2. 跑 `node scripts/publish-rust-sdks.js <VERSION>`（VERSION 是必填位置参数）

### 8.3 yank（撤回已发布版本）

如果发现严重问题，需要 yank（标记为不可用、但仍可查）：

```bash
cargo yank --version 0.1.0 digua_http_client
```

yank 后 `cargo add digua_http_client@0.1.0` 仍能拉取（已锁版本的项目不受影响），但新项目不会被推荐。需要彻底删只能联系 crates.io 管理员。

---

## 9. 故障排查

| 现象 | 原因 | 解决 |
| --- | --- | --- |
| `Permission denied (os error 13) ... target/package/tmp-registry` | `target/` 是 docker volume root-owned | 用 `CARGO_TARGET_DIR=/tmp/sdk-publish` |
| `error: no matching package named 'digua_http_client' found` | 基础库还没发布到 registry | 确保 `digua_http_client` 先发布成功再发 service SDK |
| `error: failed to load source for dependency digua_http_client` | publish 后没等索引同步就发下一个 | `digua_http_client` 发完后等 30 秒再发 service SDK |
| 脚本发完一半退出，service SDK 还是 `version` 依赖 | 脚本崩溃前未到恢复 .bak 的步骤 | 手动 `mv sdk/rust/*/Cargo.toml.bak sdk/rust/*/Cargo.toml` |
| `warning: manifest has no documentation, homepage or repository` | 缺 repository / homepage 字段 | 在 `[package]` 加上，可忽略但建议补 |

---

## 10. 相关文档

- [`./rust-sdk`](./rust-sdk)：Rust SDK 用法（消费者向）
- [`./java-integration`](./java-integration)：Java SDK 用法（对照参考）
- [`sdk/rust/README.md`](../../sdk/rust/README.md)：SDK crate 列表与测试方法
- [`scripts/publish-rust-sdks.js`](../../scripts/publish-rust-sdks.js)：发布脚本（Node.js，跨平台）
- [`scripts/publish-libs.js`](../../scripts/publish-libs.js)：发布 libs/ 共享库（Node.js，跨平台）
- [`release/dist-template/README.md`](../../release/dist-template/README.md)：离线分发包