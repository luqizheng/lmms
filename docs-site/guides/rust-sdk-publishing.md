# Rust SDK 发布指南

> 适用：把 `sdk/rust/` 下的 7 个 crate 发布到 crates.io。
> 关联：[`sdk/rust/README.md`](../../sdk/rust/README.md)（SDK 用法）、[`sdk/scripts/publish-rust-sdks.sh`](../../sdk/scripts/publish-rust-sdks.sh)（发布脚本）。
> 维护：本文档随发布脚本和 crates.io 政策变化同步更新。

---

## 1. SDK crate 总览

```
sdk/rust/
├── kudi-http-client           # 底层 HTTP + token-cache 共享库（先发布）
├── audit-service-sdk          # 6 个 service SDK 都依赖 kudi-http-client
├── config-service-sdk         # （path 依赖在本地开发时；发布时切 version）
├── file-service-sdk
├── iam-service-sdk
├── notification-service-sdk
└── token-service-sdk
```

每个 crate 独立打包为 `*.tar.gz` 上传到 [crates.io](https://crates.io)，最终用户通过 `cargo add <name>` 或 `Cargo.toml` 依赖引入。

| crate | 版本 | crates.io |
| --- | --- | --- |
| `kudi-http-client` | 0.1.0 | https://crates.io/crates/kudi-http-client |
| `iam-service-sdk` | 0.1.0 | https://crates.io/crates/iam-service-sdk |
| `token-service-sdk` | 0.1.0 | https://crates.io/crates/token-service-sdk |
| `audit-service-sdk` | 0.1.0 | https://crates.io/crates/audit-service-sdk |
| `file-service-sdk` | 0.1.0 | https://crates.io/crates/file-service-sdk |
| `notification-service-sdk` | 0.1.0 | https://crates.io/crates/notification-service-sdk |
| `config-service-sdk` | 0.1.0 | https://crates.io/crates/config-service-sdk |

---

## 2. 前置条件

发布之前请确认：

| # | 条件 | 怎么验证 |
| - | --- | --- |
| 1 | crates.io 账号 + 已验证邮箱 | 登录 https://crates.io/settings/profile，邮箱一栏显示 verified |
| 2 | 已生成 API token | https://crates.io/settings/tokens → New Token（勾选 publish 范围） |
| 3 | 本机 `cargo` 1.70+ | `cargo --version` |
| 4 | 项目级 `CARGO_TARGET_DIR` 可写 | 仓库根 `target/` 默认是 docker volume 的（root-owned），必须用 `CARGO_TARGET_DIR=/tmp/sdk-publish` 绕开 |
| 5 | 内网 mirror 配好但 sparse index 优先 | 见 §3 |

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
kudi-http-client   ←  基础库，先发
       ↑
       ├── audit-service-sdk
       ├── config-service-sdk
       ├── file-service-sdk
       ├── iam-service-sdk
       ├── notification-service-sdk
       └── token-service-sdk      ← 6 个 service SDK，每个独立，互不依赖
```

发布顺序：先发 `kudi-http-client`，等 crates.io 索引（约 30 秒）后，再发 6 个 service SDK。6 个 service SDK 之间互不依赖，可以串行或并行。

> ⚠️ 6 个 SDK 本地 Cargo.toml 用 `path = "../kudi-http-client"`（workspace path 依赖），`cargo publish` 拒绝 path 依赖。脚本会自动把它们临时改为 `version = "<x.y.z>"`，发完后恢复（见 §5.1）。

---

## 5. 发布流程

### 5.1 用发布脚本（推荐）

```bash
# 从仓库根
bash sdk/scripts/publish-rust-sdks.sh [--dry-run] [--registry <name>] [--version <ver>]
```

**参数：**

| 参数 | 说明 |
| --- | --- |
| `--dry-run` | 只验证，不上传 |
| `--registry <name>` | 上传到指定 registry（默认 crates.io），需先在 `[registries.<name>]` 配好 token |
| `--version <ver>` | 统一版本号（默认读各 crate 当前的 `version = "..."`） |
| `--help` | 打印 header 注释里的完整用法 |

**脚本做的事：**

1. **前置检查**：每个 crate 必须有 `description` + `license`，否则 exit 2。
2. **统一版本号**：所有 crate 的 `version = "..."` 统一成同一个（确保 kudi-http-client 和 service SDK 一致）。
3. **发 kudi-http-client**：无 path 依赖，直接 `cargo publish`。
4. **替换 path 依赖**：把 6 个 service SDK 的 `path = "../kudi-http-client"` 临时改为 `version = "<ver>"`，备份到 `.bak`。
5. **逐个发 service SDK**：编译 → 打包 → 上传。
6. **恢复 path 依赖**：`mv *.bak` 还原。
7. **汇总**：成功 / 失败 crate 列表。

> 💡 脚本**不**自动改 cargo mirror 配置；如果你用只读 mirror，publish 仍会卡在依赖解析（见 §5.2）。

**典型执行：**

```bash
# 第一次发布 / 验证流程
bash sdk/scripts/publish-rust-sdks.sh --dry-run

# 确认无误后真发布
CARGO_TARGET_DIR=/tmp/sdk-publish \
  CARGO_REGISTRY_TOKEN=<token> \
  bash sdk/scripts/publish-rust-sdks.sh
```

---

### 5.2 处理「只读 mirror 拦截 publish」

crates.io 被 mirror replace 后，`cargo publish` 会报：

```
error: crates-io is replaced with non-remote-registry source registry `panamax`;
include `--registry crates-io` to use crates.io
```

且即使加 `--registry crates-io`，workspace 依赖解析仍走 mirror，触发 git index 失败。**最干净的解法**：发布期间临时把 mirror 关掉。

```bash
# 1. 备份当前 config
cp .cargo/config.toml .cargo/config.toml.bak

# 2. 写一个只保留必要项的 config（无 mirror）
cat > .cargo/config.toml << 'EOF'
[net]
git-fetch-with-cli = true
EOF

# 3. publish（脚本仍然有效）
CARGO_TARGET_DIR=/tmp/sdk-publish \
  CARGO_REGISTRY_TOKEN=<token> \
  bash sdk/scripts/publish-rust-sdks.sh

# 4. 恢复
mv .cargo/config.toml.bak .cargo/config.toml
```

如果你不能动 `.cargo/config.toml`，`cargo publish` 提供 `--config` 覆盖：

```bash
cargo publish -p <crate> --allow-dirty --registry crates-io \
  --config 'source.crates-io.replace-with = ""'   # ← cargo 会拒绝（空字符串非法 source）
```

实测 cargo 1.70+ 不接受这种空字符串覆盖，且循环引用检测会挡掉 `replace-with = "crates-io"`。**推荐用临时改 config 文件的方式**（§5.2 上面那段）。

---

## 6. 发布后验证

```bash
# 拉一个临时目录试装
mkdir /tmp/sdk-verify && cd /tmp/sdk-verify
cargo init --name verify-app
cargo add iam-service-sdk kudi-http-client
cargo build           # 期望：从 crates.io 拉 kudi-http-client 和 iam-service-sdk
cargo tree | head      # 期望：能看到 kudi-http-client v0.1.0 是从 crates.io 拉的
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

`kudi-http-client` 和所有 service SDK **必须使用同一个版本号**（脚本默认行为）。原因：

- service SDK 依赖 `kudi-http-client = { version = "0.1.0" }`（发布时脚本自动注入）
- 如果 kudi-http-client 升到 `0.2.0` 而 service SDK 还在 `0.1.0`，会引入两个版本共存问题

发布流程：

1. 同步改 7 个 crate 的 `version = "..."`（可以手动或用 `cargo workspace version`）
2. 跑 `bash sdk/scripts/publish-rust-sdks.sh --version 0.2.0`

### 8.3 yank（撤回已发布版本）

如果发现严重问题，需要 yank（标记为不可用、但仍可查）：

```bash
cargo yank --version 0.1.0 kudi-http-client
```

yank 后 `cargo add kudi-http-client@0.1.0` 仍能拉取（已锁版本的项目不受影响），但新项目不会被推荐。需要彻底删只能联系 crates.io 管理员。

---

## 9. 故障排查

| 现象 | 原因 | 解决 |
| --- | --- | --- |
| `error: crates-io is replaced with non-remote-registry source registry` | 配置了只读 mirror | 见 §5.2 临时禁掉 mirror |
| `the remote server responded with an error (status 400 Bad Request): A verified email address is required` | crates.io 账号邮箱未验证 | 去 https://crates.io/settings/profile 验证邮箱 |
| `status 429 Too Many Requests` | 触发速率限制 | 见 §7，等待错误里说的时间后重试 |
| `error: failed to get anyhow as a dependency of package digua_common v0.1.0` | mirror git index 损坏 / 不同步 | 临时禁掉 mirror 直接走 crates.io；或改用 sparse index |
| `Permission denied (os error 13) ... target/package/tmp-registry` | `target/` 是 docker volume root-owned | 用 `CARGO_TARGET_DIR=/tmp/sdk-publish` |
| `warning: manifest has no documentation, homepage or repository` | 缺 repository / homepage 字段 | 在 `[package]` 加上，可忽略但建议补 |
| `error: failed to load source for dependency kudi-http-client` | publish 后没等索引同步就发下一个 | `kudi-http-client` 发完后等 30 秒再发 service SDK |
| 脚本发完一半退出，service SDK 还是 `version` 依赖 | 脚本崩溃前未到恢复 .bak 的步骤 | 手动 `mv sdk/rust/*/Cargo.toml.bak sdk/rust/*/Cargo.toml` |

---

## 10. 相关文档

- [`./rust-sdk`](./rust-sdk)：Rust SDK 用法（消费者向）
- [`./java-integration`](./java-integration)：Java SDK 用法（对照参考）
- [`sdk/rust/README.md`](../../sdk/rust/README.md)：SDK crate 列表与测试方法
- [`sdk/scripts/publish-rust-sdks.sh`](../../sdk/scripts/publish-rust-sdks.sh)：发布脚本本体
- [`release/dist-template/README.md`](../../release/dist-template/README.md)：离线分发包如何引用 crates.io 上的 SDK