# ADR-008: commit 7b62f8c 同时混入了 rsync 部署修复与 gh-proxy git config 两件事

> 日期：2026-08-04
> 状态：已记录（不修订历史）
> 关联：
> - [AGENTS.md §网络规则](../../AGENTS.md)（gh-proxy 代理约定）
> - [`.cargo/config.toml`](../../.cargo/config.toml)（`git-fetch-with-cli = true`）
> - commit `7b62f8c` — "fix(ci): ensure staging host has rsync installed"

## 上下文

`cargo audit` / `cargo deny` 在 CI runner 拉取 `https://github.com/RustSec/advisory-db.git` 时，因国内网络环境无法直连 github.com 而失败。修复方案是：

```bash
git config --global url."https://gh-proxy.org/https://github.com/".insteadOf "https://github.com/"
```

这条 `git config` 能生效，是因为 `.cargo/config.toml` 第 14 行 `git-fetch-with-cli = true` 让 cargo 拉 git source 时调用外部 `git` 命令，从而读 `~/.gitconfig` 的 `url.*.insteadOf` 重写。

**该方案在 commit `7b62f8c` 落地。** 同一个 commit 还包含另一个独立工作：给 deploy job 在 staging 主机上安装 `rsync`（commit message 描述的是这件事）。两件事改动落在同一个 commit 里：

| 主题 | 文件位置 | 描述 |
|------|---------|------|
| 部署 rsync 安装 | `.gitlab-ci.yml` deploy job 的 ssh probe 行 | 让 staging 主机有 rsync，否则 rsync over ssh 协议在远端找不到命令 |
| CI advisory-db 走 gh-proxy | `.gitlab-ci.yml` cargo_audit / cargo_deny 的 `before_script` | 让 cargo-audit 拉 RustSec/advisory-db 不再失败 |

## 为什么 commit message 只提一件事

`7b62f8c` 的 message 是 `fix(ci): ensure staging host has rsync installed` —— 只描述了 rsync 这一件事，**没有提 gh-proxy 改 advisory-db**。这是违反 [AGENTS.md](../../AGENTS.md)「一个任务一个 commit」原则的失误。

## 为什么不在事后修订这个 commit

AGENTS.md 明确禁止 `amend` / `force-push`：

> 一个任务一个 commit；禁止 amend / force-push。

并且 `7b62f8c` 已经在 `origin/main` 上，后续 commit（`23206a2`、`1ab1069`、`e0db5b0`、`4ad5219`）都基于它。修订它需要 `force-push` 整条 main，**不在可选方案内**。

候选方案对比：

| 方案 | 代价 | 是否采纳 |
|------|------|----------|
| 保持现状 | 未来 `git blame` 7b62f8c 会跑偏（rsync 行能定位到正确 commit message；gh-proxy 行虽然属于同一个 commit 但 message 不提） | ✅ 已采纳 |
| `git revert -n 7b62f8c -- .gitlab-ci.yml` 仅 revert 4 行 gh-proxy，再单独 commit | 引入一条 revert + 一条重做，净增 2 条噪音 commit | ❌ |
| `git rebase -i 7b62f8c^` 拆成两条再 `push --force-with-lease` | 违反「禁止 force-push」规则 | ❌ |

## 决议

**接受 `7b62f8c` 现状，不修订历史。** 未来读 `.gitlab-ci.yml` 第 657-660 行 / 第 683-684 行的 `git config --global url.*.insteadOf` 时，应当知道：

1. 这两段配置来自 commit `7b62f8c`
2. 该 commit 的 message 是 "ensure staging host has rsync installed"，**与这 4 行的实际意图不一致**
3. 不要因为 message 不符就误判为错改 —— 这是"混在同一个 commit"的产物，不是错误
4. 真实原因和取舍见本 ADR

## 后续约束（仅作提醒，不构成规则变更）

- 未来如果再有类似的多主题改动：**先开任务分支 + 一个主题一条 commit**，避免重复本 ADR 的场景
- 如果发现又产生"被迫接受混合 commit"的情况，应在合并前发现并拒绝，而非合并后追加 ADR
