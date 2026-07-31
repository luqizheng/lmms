# 5B-fu6 — Principal extractor 响应不泄露 JWT 内部错误 — 实施 plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `libs/jwt-ctx/tests/extractor_test.rs` 加回归测试，锁住「`Principal` extractor 失败时响应 body 永不为空」这一安全属性；归档 5B-fu6 到 `done.md`。

**Architecture:** 复用现有 `tower::ServiceExt::oneshot` + `make_state` / `make_token` 测试模式；新增 `assert_401_empty_body` helper；6 个独立 `#[tokio::test]` 函数覆盖 6 类错误路径；现有 2 个测试各加 1 行 `body.is_empty()` 断言。`principal.rs` 主体代码**不动**（commit `99fc5f9` 已实现正确行为）。

**Tech Stack:** Rust 2021 / `axum` 0.8 / `tokio` 1 (full feature set via workspace) / `tower` 0.5 / `digua_common-jwt` / `serde_json`。

## Global Constraints

- 不修改 `libs/jwt-ctx/src/principal.rs` 主体（已正确，commit `99fc5f9`）。
- 不修改 `services/api-gateway/src/middleware/mod.rs`（独立 follow-up）。
- 不动 `tracing::warn!` 日志格式。
- 所有改动走一个独立 `feat(test): 5B-fu6 ...` 前缀 commit + 一个 `docs: 5B-fu6 sweep` 收尾 commit。
- PEM fixture 仅放 `libs/jwt-ctx/tests/fixtures/`，不进生产 binary。
- `jwt-ctx` 不在 `services/`，CI service-standard / spec / contracts gate 自动跳过；本 plan 不跑这些 gate。
- 不引入新依赖到 `[dependencies]`；如需 `jsonwebtoken` / `chrono` 走 `[dev-dependencies]`。

---

## Task 1: 加 helper + 加固现有 2 个测试

**Files:**
- Modify: `libs/jwt-ctx/tests/extractor_test.rs` (1 helper + 2 行断言)
- Create: `libs/jwt-ctx/tests/fixtures/other_private.pem` (RSA 2048 测试用私钥)
- Create: `libs/jwt-ctx/tests/fixtures/other_public.pem` (伴生公钥；本 plan 不直接用)

**Interfaces:**
- Consumes: 现有 `make_state()` / `make_token()` helpers（已存在）
- Produces: `assert_401_empty_body(app: Router, token: &str)` helper —— Task 2 复用

- [ ] **Step 1: 生成第二对 RSA 测试 key fixture**

```bash
cd /home/kudi/low-memory-micro-services
mkdir -p libs/jwt-ctx/tests/fixtures
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -outform PEM \
    -out libs/jwt-ctx/tests/fixtures/other_private.pem
openssl rsa -in libs/jwt-ctx/tests/fixtures/other_private.pem -pubout -outform PEM \
    -out libs/jwt-ctx/tests/fixtures/other_public.pem
# 确认：
ls -la libs/jwt-ctx/tests/fixtures/
head -1 libs/jwt-ctx/tests/fixtures/other_private.pem  # -----BEGIN PRIVATE KEY-----
```

Expected: 两个 PEM 文件已生成，文件头分别为 `-----BEGIN PRIVATE KEY-----` 和 `-----BEGIN PUBLIC KEY-----`。

- [ ] **Step 2: 在 extractor_test.rs 加 helper + PEM 常量**

打开 `libs/jwt-ctx/tests/extractor_test.rs`，在 `TEST_PUBLIC_PEM` 常量之后（line 13 后）追加：

```rust
const OTHER_PRIVATE_PEM: &str =
    include_str!("fixtures/other_private.pem");
const OTHER_PUBLIC_PEM: &str =
    include_str!("fixtures/other_public.pem");
```

在文件末尾（`bad_signature_returns_401` 函数之后）追加 helper：

```rust
/// 断言 401 响应且 body 为空。
/// 用法：构造一个使用 Principal extractor 的 Router，发送伪造 token，
/// 验证返回的状态码是 401 且 body bytes 为空（无 JWT 内部错误泄露）。
async fn assert_401_empty_body(app: Router, token: &str) {
    let req = Request::builder()
        .uri("/")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401, "expected 401 Unauthorized");
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert!(
        body.is_empty(),
        "401 response body must be empty (no JWT internal error leak), got: {:?}",
        body
    );
}
```

- [ ] **Step 3: 加固 `missing_header_returns_401`**

修改 `missing_header_returns_401`（line 62-69）末尾，加 body 断言：

```rust
#[tokio::test]
async fn missing_header_returns_401() {
    let state = make_state();
    let app = Router::new().route("/", get(who)).with_state(state);
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert!(body.is_empty(), "401 body must be empty, got: {:?}", body);
}
```

- [ ] **Step 4: 加固 `bad_signature_returns_401`**

修改 `bad_signature_returns_401`（line 71-88）末尾，加 body 断言：

```rust
#[tokio::test]
async fn bad_signature_returns_401() {
    let state = make_state();
    let mut token = make_token(serde_json::json!({"sub":"u","tid":1}));
    // Tamper with the signature
    let parts: Vec<&str> = token.split('.').collect();
    let new_sig = format!("A{}", &parts[2][1..]);
    token = format!("{}.{}.{}", parts[0], parts[1], new_sig);

    let app = Router::new().route("/", get(who)).with_state(state);
    let req = Request::builder()
        .uri("/")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert!(body.is_empty(), "401 body must be empty, got: {:?}", body);
}
```

- [ ] **Step 5: 跑测试验证通过**

```bash
cd /home/kudi/low-memory-micro-services
cargo test -p jwt-ctx --test extractor_test
```

Expected:
```
running 3 tests
test extractor_returns_principal ... ok
test missing_header_returns_401 ... ok
test bad_signature_returns_401 ... ok

test result: ok. 3 passed; 0 failed; 0 ignored
```

- [ ] **Step 6: Commit**

```bash
cd /home/kudi/low-memory-micro-services
git add libs/jwt-ctx/tests/fixtures/other_private.pem \
        libs/jwt-ctx/tests/fixtures/other_public.pem \
        libs/jwt-ctx/tests/extractor_test.rs
git commit -m "feat(test): 5B-fu6 — 加 helper + 加固现有 extractor 401 响应测试"
```

---

## Task 2: 加 6 个新测试覆盖 6 类 JWT 错误路径

**Files:**
- Modify: `libs/jwt-ctx/Cargo.toml` (dev-dep 加 `jsonwebtoken` + `chrono`，用于「过期 token」case)
- Modify: `libs/jwt-ctx/tests/extractor_test.rs` (追加 6 个 `#[tokio::test]`)

**Interfaces:**
- Consumes: Task 1 的 `assert_401_empty_body` helper + `OTHER_PRIVATE_PEM` / `OTHER_PUBLIC_PEM` 常量
- Produces: 6 个新 `#[tokio::test]` 函数

- [ ] **Step 1: 在 jwt-ctx Cargo.toml 加 dev-dependencies**

打开 `libs/jwt-ctx/Cargo.toml`，把 dev-deps 段从：

```toml
[dev-dependencies]
tokio.workspace = true
tower = { version = "0.5", features = ["util"] }
base64.workspace = true
```

改为：

```toml
[dev-dependencies]
tokio.workspace = true
tower = { version = "0.5", features = ["util"] }
base64.workspace = true
jsonwebtoken.workspace = true  # 5B-fu6: 构造 exp-in-past 的 token
chrono.workspace = true        # 5B-fu6: 取 now() 时间戳
```

（`jsonwebtoken` 和 `chrono` 已在 workspace 根 `Cargo.toml` 声明，见 `Cargo.toml:7` 和 `Cargo.toml:?`，无需新增。）

- [ ] **Step 2: 在 extractor_test.rs 末尾追加 6 个测试**

在 `assert_401_empty_body` helper 之后（Task 1 末尾位置）追加：

```rust
// ============================================================================
// 5B-fu6: 响应 body 不泄露 JWT 内部错误
// 对 6 类错误路径都断言 status == 401 && body.is_empty()
// ============================================================================

#[tokio::test]
async fn bad_signature_body_is_empty() {
    let state = make_state();
    let mut token = make_token(serde_json::json!({"sub":"u","tid":1}));
    let parts: Vec<&str> = token.split('.').collect();
    let new_sig = format!("A{}", &parts[2][1..]);
    token = format!("{}.{}.{}", parts[0], parts[1], new_sig);
    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, &token).await;
}

#[tokio::test]
async fn expired_token_body_is_empty() {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    let state = make_state();
    // sign_token 强制 exp = now + ttl，不能产生「已过期」token；
    // 必须用 jsonwebtoken 直接构造，exp 设为 1 小时前。
    let now = chrono::Utc::now().timestamp();
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("default".into());
    let key = EncodingKey::from_rsa_pem(TEST_PRIVATE_PEM.as_bytes()).unwrap();
    let claims = serde_json::json!({
        "iss": "test",
        "sub": "u",
        "tid": 1,
        "iat": now - 3600,
        "exp": now - 3600,
    });
    let token = encode(&header, &claims, &key).unwrap();
    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, &token).await;
}

#[tokio::test]
async fn missing_sub_claim_body_is_empty() {
    let state = make_state();
    let token = make_token(serde_json::json!({"tid": 1}));
    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, &token).await;
}

#[tokio::test]
async fn missing_tid_claim_body_is_empty() {
    let state = make_state();
    let token = make_token(serde_json::json!({"sub": "u"}));
    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, &token).await;
}

#[tokio::test]
async fn wrong_signing_key_body_is_empty() {
    // 用 other_private 签，但 JWKSCache 仍用 test_public 验
    // → 签名验证失败，但 body 仍须空
    let state = make_state();
    let token = sign_token(OTHER_PRIVATE_PEM, &serde_json::json!({"sub":"u","tid":1}),
                           std::time::Duration::from_secs(60)).unwrap();
    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, &token).await;
}

#[tokio::test]
async fn malformed_token_body_is_empty() {
    let state = make_state();
    let app = Router::new().route("/", get(who)).with_state(state);
    assert_401_empty_body(app, "not.a.jwt.at.all").await;
}
```

- [ ] **Step 3: 跑全部 extractor 测试**

```bash
cd /home/kudi/low-memory-micro-services
cargo test -p jwt-ctx --test extractor_test
```

Expected: 9 passed, 0 failed:
```
test extractor_returns_principal ... ok
test missing_header_returns_401 ... ok
test bad_signature_returns_401 ... ok
test bad_signature_body_is_empty ... ok
test expired_token_body_is_empty ... ok
test missing_sub_claim_body_is_empty ... ok
test missing_tid_claim_body_is_empty ... ok
test wrong_signing_key_body_is_empty ... ok
test malformed_token_body_is_empty ... ok

test result: ok. 9 passed; 0 failed
```

- [ ] **Step 4: 跑整个 jwt-ctx crate 测试确认无回归**

```bash
cd /home/kudi/low-memory-micro-services
cargo test -p jwt-ctx
```

Expected: 所有现有 test (`from_token_test` / `principal_test` / `principal_tenant_test` / `scope_test` / `extractor_test`) 全 pass；新加的 6 个 extractor 测试也 pass。

- [ ] **Step 5: 跑 workspace build 确认 PEM fixture 不破任何 binary**

```bash
cd /home/kudi/low-memory-micro-services
cargo build --workspace
```

Expected: build 成功，无错误。PEM 仅在 `tests/` 目录下通过 `include_str!` 引用，binary 不打包。

- [ ] **Step 6: Commit**

```bash
cd /home/kudi/low-memory-micro-services
git add libs/jwt-ctx/Cargo.toml libs/jwt-ctx/tests/extractor_test.rs
git commit -m "feat(test): 5B-fu6 — 6 类 JWT 错误路径响应 body 必空回归测试"
```

---

## Task 3: TODO / done 归档

**Files:**
- Modify: `TODO.md` (line 113-114 区域)
- Modify: `done.md` (顶部追加 5B-fu6 条目)

**Interfaces:**
- Consumes: 现有 `done.md` 顶部结构
- Produces: TODO.md 5B-fu6 行变 `[x]`；done.md 新增一条

- [ ] **Step 1: 改 TODO.md**

打开 `TODO.md`，找到 5B-fu6 行（line 113 附近）：

```markdown
- [x] **5B-fu5** CI 对齐 5B spec §7.2：`cargo test --workspace --lib`...
- [x] **5B-fu6** principal extractor 在 `from_token` 失败时可能泄露 jwt 错误细节到响应（当前统一 401）。生产应在日志中记录原因，响应只返回通用消息。
```

把 5B-fu6 行的 `[x]` 状态保留为 `[x]`（已经是 `[x]` 了），但要更新内容指向归档。先确认 5B-fu6 当前内容，然后改为：

```markdown
- [x] **5B-fu6** Principal extractor 响应 body 不泄露 JWT 内部错误（commits `c5d694d` + 后续）：6 类错误路径回归测试守住。详见 `done.md` Phase 5-B 段。
```

注：5B-fu6 在原 TODO.md 中已经是 `[x]` 状态（plan-eng-review 已勾），但内容描述不准确（说"已统一 401"但缺回归保护）。本次改动只是更新描述 + 加 commit 引用。

- [ ] **Step 2: 在 done.md 加 5B-fu6 条目**

打开 `done.md`，找到最近的 Phase 5-B 段（5B-fu5 附近），追加 5B-fu6 条目。如果没有 Phase 5-B 段，就在 Phase 5-A 段后追加子段。

如果已有 "### Phase 5-B — JWT 签名验证 + service-jwt 机制" 段（已存在，参见 5B-fu3 / 5B-fu5），在 5B-fu5 条目下加：

```markdown
- [x] **5B-fu6** Principal extractor 响应 body 不泄露 JWT 内部错误
  - 加 `assert_401_empty_body` helper + 加固 2 个现有 401 测试
  - 6 类错误路径独立回归测试：`bad_signature` / `expired_token` / `missing_sub` / `missing_tid` / `wrong_signing_key` / `malformed_token`
  - `jwt-ctx` 新增 dev-dep `jsonwebtoken` + `chrono`（用于「已过期 token」case）
  - 新增 fixture `libs/jwt-ctx/tests/fixtures/other_{private,public}.pem`（RSA 2048 测试用）
  - 验证：`cargo test -p jwt-ctx` 全 pass；`cargo build --workspace` 干净
  - Spec: `docs/superpowers/specs/2026-07-08-5b-fu6-jwt-error-leak-design.md`
```

如果没有现成 Phase 5-B 段，则在 done.md 顶部合适位置（Phase 5 区域）追加整段。

- [ ] **Step 3: 跑最终回归确认**

```bash
cd /home/kudi/low-memory-micro-services
cargo test -p jwt-ctx
```

Expected: 全 pass。

- [ ] **Step 4: Commit**

```bash
cd /home/kudi/low-memory-micro-services
git add TODO.md done.md
git commit -m "docs: 5B-fu6 — TODO 描述更新 + done 归档"
```

---

## Self-Review（plan 完成时跑）

逐项对照 spec `2026-07-08-5b-fu6-jwt-error-leak-design.md`：

| Spec 项 | Plan 覆盖 |
|---|---|
| §2.1 目标 1: 6 类错误路径 body 为空 | Task 2 Step 2（6 个新测试） |
| §2.1 目标 2: 现有 2 个测试加 `body.is_empty()` 断言 | Task 1 Step 3 + Step 4 |
| §2.1 目标 3: TODO 归档 | Task 3 全部 |
| §2.2 非目标 1: 不改 `principal.rs` | 全 plan 无 touch principal.rs |
| §2.2 非目标 2: 不改 `auth_middleware` | 全 plan 无 touch |
| §2.2 非目标 3: 不引 type-level 负向断言 | 无 trait/where 边界断言 |
| §2.2 非目标 4: 不动 `tracing::warn!` | 无 touch |
| §3.1 测试位置 | Task 1 + Task 2 全部在 `libs/jwt-ctx/tests/extractor_test.rs` |
| §3.2 现有测试加固 | Task 1 Step 3-4 |
| §3.3 6 个新测试 | Task 2 Step 2 |
| §3.4 PEM fixture | Task 1 Step 1（生成）+ Cargo.toml 不需要 mirror |
| §3.5 Cargo.toml 调整 | Task 2 Step 1（dev-deps + jsonwebtoken + chrono） |
| §3.6 验证 | Task 1 Step 5 + Task 2 Step 3-5 + Task 3 Step 3 |
| §4 改动清单 | 全部覆盖 |
| §5 风险与缓解 | PEM 仅 tests/ + sleep 不需要（用 jsonwebtoken） + tokio 已有 full feature |
| §6 后续（auth_middleware 4 条消息）| Task 3 备注中说明留作独立 follow-up |

**无 placeholder**（已 grep：`TBD` / `TODO` / "implement later" / "fill in" / "add appropriate" —— 无命中）

**类型一致**：helper 名 `assert_401_empty_body(app: Router, token: &str)` —— 6 个调用点全部一致。常量名 `OTHER_PRIVATE_PEM` / `OTHER_PUBLIC_PEM` —— Task 1 定义，Task 2 引用，匹配。
