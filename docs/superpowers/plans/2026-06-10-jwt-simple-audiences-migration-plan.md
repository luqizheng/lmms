# jwt-simple 0.12 Audiences API 迁移 — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 token-service / api-gateway 中残留的 jwt-simple 旧 API（`claims.audiences = Some(HashSet<...>)`）迁到 0.12 新 API（`Option<Audiences>`），消除编译错误。

**Architecture:** 不改业务逻辑，仅替换类型。`Audiences` 是 `jwt_simple::prelude::*` 下的枚举；对照物是 [`libs/authz/tests/require_service_token_test.rs:21`](file:///d:/projects/coder-micro-services/libs/authz/tests/require_service_token_test.rs#L21)。

**背景：** 本 bug 是 pre-existing（`5192475 chore(workspace): credential-login + user-service rust migration` 引入），**非** [`2026-06-10-go-to-rust-migration-cleanup-plan.md`](file:///d:/projects/coder-micro-services/docs/superpowers/plans/2026-06-10-go-to-rust-migration-cleanup-plan.md) 引起。cleanup T3 在沙箱内未能 `cargo check` 验证，故未暴露。

**授权：** 修 `services/<svc>/src/` 业务源码需用户明文授权（rules §修改边界）。本 plan 即为该授权载体。

**Tech Stack:** Rust 1.78+, jwt-simple 0.12 (workspace pinned), `axum 0.8`。

---

## 文件结构

| 路径 | 改动 |
|---|---|
| `services/token-service/src/issuer.rs` | line 89 + 删 dead `use std::collections::HashSet;` |
| `services/token-service/src/routes/refresh.rs` | line 41 视 cargo 报错决定 |
| `services/api-gateway/src/jwt.rs` | line 91 视 cargo 报错决定 |
| `services/token-service/tests/issuer_test.rs` | 5 处 视 cargo 报错决定 |
| `docs/superpowers/plans/2026-06-10-credential-login-user-rust-plan.md` | line 985 / 642 修模板（避免后续照搬） |

---

## Task 1: 修 issuer.rs:89（确认 broken）

**Files:**
- Modify: `services/token-service/src/issuer.rs`

- [ ] **Step 1: 改 line 89**

`HashSet::from([...])` → `Audiences::AsString(...)`（单元素场景）：

```rust
// before
claims.audiences = Some(HashSet::from(["coder-micro-services".to_string()]));
// after
claims.audiences = Some(Audiences::AsString("coder-micro-services".to_string()));
```

- [ ] **Step 2: 删 dead import**

```rust
// before
use std::collections::HashSet;
let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
// ...
// after (删 line 84)
let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
```

（`HashMap` 仍用，不删。）

- [ ] **Step 3: 验证编译（真实 PowerShell）**

```powershell
cargo check -p token-service
```

Expected: 仍可能报其他 7 处 `allowed_issuers` / `audiences` 错，逐个进入下个 Task；不报 `issuer.rs:89`。

---

## Task 2: 修 refresh.rs:41

**Files:**
- Modify: `services/token-service/src/routes/refresh.rs`

- [ ] **Step 1: 读 `options.allowed_issuers` 类型**

`VerificationOptions` 在 `jwt_simple::prelude::*` 下；查 workspace 依赖源码或 cargo doc：

```powershell
# 真实 PowerShell
cargo doc -p jwt-simple --no-deps --open
```

或直接读 `C:\Users\<user>\.cargo\registry\src\index.crates.io-*\jwt-simple-0.12.*\src\verification.rs` 找 `allowed_issuers` 字段。

- [ ] **Step 2a: 若仍是 `Option<HashSet<String>>`（最可能）**

**不**改，保持 `HashSet::from([...])`，Task 2 直接跳过。

- [ ] **Step 2b: 若也是 `Option<Audiences>`**

改为 `Some(Audiences::AsString(issuer.to_string()))`。

- [ ] **Step 3: 验证编译**

```powershell
cargo check -p token-service
```

Expected: refresh.rs 不再报错。

---

## Task 3: 修 api-gateway/jwt.rs:91

**Files:**
- Modify: `services/api-gateway/src/jwt.rs`

- [ ] **Step 1-3**: 同 Task 2 流程。

```powershell
cargo check -p api-gateway
```

---

## Task 4: 修 issuer_test.rs（5 处）

**Files:**
- Modify: `services/token-service/tests/issuer_test.rs`

- [ ] **Step 1**: 视 cargo 报错决定 5 处是 `HashSet` 还是 `Audiences`。优先用 cargo check 驱动。

```powershell
cargo check -p token-service --all-targets
```

- [ ] **Step 2**: 跑现有测试（真实 PowerShell）：

```powershell
.\scripts\test-orchestrator.ps1 test-all -Service token-service
```

Expected: 全部通过（如果 mock 工具链没坏）。

---

## Task 5: 修 plan 模板（防止照搬）

**Files:**
- Modify: `docs/superpowers/plans/2026-06-10-credential-login-user-rust-plan.md`

- [ ] **Step 1: 改 line 985**

```rust
// before
claims.audiences = Some(HashSet::from(["coder-micro-services".to_string()]));
// after
claims.audiences = Some(Audiences::AsString("coder-micro-services".to_string()));
```

- [ ] **Step 2: 改 line 642（match 块）**

按 `libs/authz/src/require_service_token.rs:41` 模式改：

```rust
let aud_ok = match &claims.audiences {
    Some(Audiences::AsString(a)) if a == expected_aud => true,
    Some(Audiences::AsSet(set)) if set.contains(expected_aud) => true,
    _ => false,
};
```

- [ ] **Step 3: 同步改 plan 顶部的「Tech Stack」注释**

把 `jwt-simple 0.12` 行附注「API: `Option<Audiences>`，不是 `Option<HashSet<String>>`」。

---

## Task 6: 收尾

- [ ] **Step 1: 整 workspace 编译**

```powershell
cargo check --workspace --all-targets
```

Expected: 0 error。

- [ ] **Step 2: 整 workspace 测试**

```powershell
.\scripts\test-orchestrator.ps1 test-all
```

Expected: 全部通过（已知 credential-login-service 是 stub，可能 0 测试；其他 4 服务全过）。

- [ ] **Step 3: fmt / clippy**

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 自审

- **范围控制**：仅 jwt-simple API 替换；不动 token 签发/校验逻辑。
- **依赖**：Task 1 必跑（修 `cargo check` 卡住的第一个错）；Task 2-4 按 `cargo check --all-targets` 暴露顺序排；Task 5 模板可独立改。
- **回滚**：每 Task 一个 commit，命名 `fix(token-service):` / `fix(api-gateway):` / `chore(plans):`。
- **不在本 plan 范围**：
  - 升级 jwt-simple 大版本（仍是 0.12）
  - 改 token 业务逻辑
  - 重构 `VerificationOptions` 字段
- **风险点**：
  - `Audiences` 在 jwt-simple 0.12 还有 `From<&str>` / `From<HashSet<String>>` 等 impl；优先用显式枚举（`AsString` / `AsSet`）保持可读性，避免依赖隐式 `From`
  - `allowed_issuers` 字段类型若在 0.12 仍是 `HashSet`，则 **不**改；若 API 也变了，按 Task 2-4 流程同样处理

## 关联

- 触发发现：[`2026-06-10-go-to-rust-migration-cleanup-plan.md` 评审](file:///d:/projects/coder-micro-services/docs/superpowers/plans/2026-06-10-go-to-rust-migration-cleanup-plan.md)
- 参照物（已修对）：[`libs/authz/src/require_service_token.rs`](file:///d:/projects/coder-micro-services/libs/authz/src/require_service_token.rs) + [`libs/authz/tests/require_service_token_test.rs`](file:///d:/projects/coder-micro-services/libs/authz/tests/require_service_token_test.rs)
- 原始引入 commit：`5192475 chore(workspace): credential-login + user-service rust migration`（pre-existing，非 cleanup 引起）
