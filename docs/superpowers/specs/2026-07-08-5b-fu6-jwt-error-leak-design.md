# 5B-fu6 — Principal extractor 响应不泄露 JWT 内部错误 — 设计

> 日期：2026-07-08
> 来源：`TODO.md` Phase 5-B 后续项「**5B-fu6** principal extractor 在
> `from_token` 失败时可能泄露 jwt 错误细节到响应（当前统一 401）。生产应在
> 日志中记录原因，响应只返回通用消息。」
> 前置：commit `99fc5f9`（`dephase(auth): deprecate from_jwt_payload; add warn
> logging on verify failure`）已经把 extractor 改成「`tracing::warn!(error = %err, ...)`
> → bare `StatusCode::UNAUTHORIZED`」。本 spec 不改实现，只加回归测试守住
> 这一性质，并把 TODO 移归档。
> 范围：仅 `libs/jwt-ctx`；不动 `services/api-gateway/src/middleware/mod.rs`
> 的 `auth_middleware`（其 4 条具体错误消息是独立 follow-up，本次不做）。

---

## 1. 背景与现状

### 1.1 当前实现

`libs/jwt-ctx/src/principal.rs:179-198` `Principal::from_request_parts`：

```rust
type Rejection = axum::http::StatusCode;
async fn from_request_parts(...) -> Result<Self, Self::Rejection> {
    // ...
    Self::from_token(token, &cache, None).await.map_err(|err| {
        tracing::warn!(error = %err, "jwt verification failed, returning 401");
        axum::http::StatusCode::UNAUTHORIZED
    })
}
```

`PrincipalVerifyError`（含 `VerifyFailed(VerifyError)` / `MissingSub` / `MissingTenantId`
等变体，错误细节可能包含 `kid` / `malformed JWT: <jsonwebtoken 内部错误>`
等敏感信息）**没有** `IntoResponse` impl —— axum 把 `StatusCode` rejection
渲染成空 body 401。响应已经统一不带 JWT 内部错误。

### 1.2 风险

1. **回归风险**：未来有人改 `Rejection` 类型（如换成自定义 struct 带
   `IntoResponse` impl 把 error 序列化进 body），目前没有测试守。
2. **审计缺口**：现有 `libs/jwt-ctx/tests/extractor_test.rs` 只断言 `status == 401`，
   不检查 body —— 验证不充分。
3. **范围蔓延预防**：TODO 的字面描述是「principal extractor」，但实际还有
   `services/api-gateway/src/middleware/mod.rs` 的 `auth_middleware` 暴露 4
   条具体消息（`"Invalid or expired token"` / `"missing tenant claim"` /
   `"Token revoked"` / `"permissions changed, re-authenticate"`）。后者是
   独立安全议题（泄露「哪个 check 失败」），本 spec 不收，留作独立 follow-up。

---

## 2. 目标 / 非目标

### 2.1 目标

1. **`Principal` extractor 失败时响应 body 为空**：6 类错误路径都断言
   `status == 401 && body.is_empty()`。
2. **现有测试加固**：`missing_header_returns_401` /
   `bad_signature_returns_401` 各加 `body.is_empty()` 断言。
3. **TODO 归档**：5B-fu6 标记完成，移到 `done.md`。

### 2.2 非目标

- 不改 `principal.rs`（已正确）。
- 不改 `auth_middleware`（独立 follow-up，本次不做）。
- 不引入 type-level 负向断言（`where Self::Rejection = StatusCode`）——
  现有测试覆盖足够，trick 引入复杂度但无新增保护。
- 不动 `tracing::warn!`（已含完整 error context）。
- 不动 gateway / 其他服务。

---

## 3. 设计

### 3.1 测试位置

`libs/jwt-ctx/tests/extractor_test.rs` —— 复用现有 `make_state` / `make_token` /
`tower::ServiceExt::oneshot` 模式。

### 3.2 现有测试加固

```rust
#[tokio::test]
async fn missing_header_returns_401() {
    // ... existing setup ...
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert!(body.is_empty(), "401 body must be empty, got: {:?}", body);
}

#[tokio::test]
async fn bad_signature_returns_401() {
    // ... existing setup ...
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert!(body.is_empty(), "401 body must be empty, got: {:?}", body);
}
```

### 3.3 新测试 `extractor_response_body_is_empty_on_all_jwt_errors`

6 个 sub-case，参数化表：

| # | 错误路径 | 构造方式 |
|---|---|---|
| 1 | 签名错 | 复用 3.2 模式（`A{rest_of_sig}`） |
| 2 | token 过期 | `sign_token` 用 `Duration::from_secs(0)`，调用前 `tokio::time::sleep(Duration::from_millis(1100))` 越过 exp 边界；或直接构造 `exp: 1` claims + 用 `sign_token` 内部覆盖（需读源码确认 `iat/exp` 是否被覆盖 → 源码 `libs/common-jwt/src/verify.rs:37-40` 显示 `iat` / `exp` 都被覆盖，故必须 sleep） |
| 3 | 缺 sub | `sign_token` 时 claims 无 `sub` |
| 4 | 缺 tid | `sign_token` 时 claims 无 `tid` |
| 5 | 错误签发 key | 测试 fixture 引入第二对 PEM（`tests/fixtures/other_private.pem` + `other_public.pem`），用它签 token、用 `TEST_PUBLIC_PEM` 验 |
| 6 | 格式坏 | 直接 `format!("Bearer {}", "not.a.jwt.at.all")` |

每个 sub-case 共享 assertion helper：

```rust
async fn assert_401_empty_body(app: Router, token_header_value: String) {
    let req = Request::builder()
        .uri("/")
        .header(header::AUTHORIZATION, token_header_value)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401, "expected 401");
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert!(body.is_empty(), "401 body must be empty, got: {:?}", body);
}
```

6 个独立 `#[tokio::test]` 函数（不上 table-driven，避免 panic message 难读）。

### 3.4 第二对 RSA key fixture

仅 `libs/jwt-ctx/tests/fixtures/` 下新增（`common-jwt` 不需要 mirror ——
它的 `sign_token` 只接 `&str` PEM 参数，公钥在 `JWKSCache::from_pem` 时由
jwt-ctx 测试自己加载）：

```bash
# 生成新 key pair（不重用 test_private.pem）：
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -outform PEM \
    -out libs/jwt-ctx/tests/fixtures/other_private.pem
openssl rsa -in libs/jwt-ctx/tests/fixtures/other_private.pem -pubout -outform PEM \
    -out libs/jwt-ctx/tests/fixtures/other_public.pem
```

`other_public.pem` 本 spec **不用**（仅作为 `other_private.pem` 的伴生文件
保留，test case 5 只需签发方私钥，验证方仍用 `TEST_PUBLIC_PEM`）。如不想
留冗余，可以只生成 `other_private.pem`。

注：fixture 文件**只放测试目录**，不进生产构建。`include_str!` 在
`tests/` 下生效，binary 不打包。

### 3.5 Cargo.toml 调整

`libs/jwt-ctx/Cargo.toml` dev-dependencies 需补：

```toml
[dev-dependencies]
# 现有保持
tokio = { workspace = true, features = ["time", "macros", "rt"] }  # 增 "time" feature（已可能已有）
```

检查现有 `tokio.workspace = true` 是否已带 `time` feature（看 workspace root
`Cargo.toml`）；如未带则只在本 crate 的 dev-dep 覆盖。

### 3.6 验证

```bash
# lib 全测
cargo test -p jwt-ctx --lib

# 集成测（包含新测试）
cargo test -p jwt-ctx --tests extractor_test

# workspace build（确保 fixture include 不破坏任何 binary）
cargo build --workspace

# lint / contracts
node scripts/check-service-standard.js    # jwt-ctx 不在 services/，应 skip
node scripts/contracts-gen.js --check     # 不改 contracts，跳过
```

无 e2e / testcontainers 依赖（纯 in-process 测试）。

---

## 4. 改动清单

| 文件 | 改动 |
|---|---|
| `libs/jwt-ctx/tests/extractor_test.rs` | +6 个新 `#[tokio::test]` + 2 个现有测试加 1 行断言 + 1 个 `assert_401_empty_body` helper |
| `libs/jwt-ctx/tests/fixtures/other_private.pem` | 新增（RSA 2048 测试签发方私钥） |
| `libs/jwt-ctx/tests/fixtures/other_public.pem` | 新增（伴生文件，spec 不用；可省） |
| `libs/jwt-ctx/Cargo.toml` | dev-dep tokio 加 `time` feature（如缺） |
| `TODO.md` | 5B-fu6 移到 `done.md` |
| `done.md` | 加 5B-fu6 条目 |

预计 diff：~100 行（含 PEM 文本），实际 Rust 代码 ≤ 60 行。

---

## 5. 风险与缓解

| 风险 | 缓解 |
|---|---|
| PEM fixture 进 git 后被复用为生产 key | 仅放 `tests/fixtures/`，`include_str!` 仅在 tests/ 下；prod 路径不编译 |
| `tokio::time::sleep(1.1s)` 让 CI 慢 | 单测只跑 1 次 ~1.1s，CI 可接受（项目其他测试已有 longer sleeps） |
| 6 个独立测试名重复样板 | `assert_401_empty_body` helper 收口；测试名区分错误路径，panic 时易读 |
| 现有 2 个测试加 `body.is_empty()` 改坏其他预期 | 现有 401 测试本就不期望 body；加断言只是加强 |

---

## 6. 后续（独立 follow-up，不在本次范围）

- `services/api-gateway/src/middleware/mod.rs` `auth_middleware` 4 条具体错误
  消息合并为 1 条通用「Unauthorized」，配合 tracing 记录细节（已记在
  `TODO.md` 不在 5B-fu6 内）。
