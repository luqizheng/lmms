# Rust CI 合并 + fmt/contracts 修复

日期：2026-08-20
分支：`feat/rust-ci-merge`
范围：`.gitlab-ci.yml`（test stage）、`contracts/codegen/*.js`（输出格式）、`libs/authz/src/perms.rs` + `libs/common-event/src/events.rs` + `libs/common-error/src/codes.rs`（重新生成）、243 个 fmt 修复文件

## 背景

`test` stage 当前有 4 个 job 都在 builder 镜像（`zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl`）内编译 workspace：

| Job | 实际编译命令 |
|---|---|
| `rust_test` | `cargo test --workspace --lib` |
| `cargo_clippy` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `cargo_fmt_check` | `cargo fmt --check`（不编译） |
| `spec_generate` | `cargo run --bin dump_openapi`（7 个 service × 1 bin target） |

GitLab docker runner 的 job 间不共享文件系统，`rust_test` 还主动 `cargo clean`（workaround 磁盘 quota），导致每次都从头编译整个 workspace。

## 目标

1. 把上述 4 个 job 合并为单一 `rust_ci` job
2. 在单次 container 内顺序执行 `cargo fmt --check` → `cargo clippy --workspace --all-targets` → `cargo run --bin dump_openapi` → `cargo test --workspace --lib`
3. 共享同一个 `target/`，后续步骤复用前一步的编译产物（增量）
4. fmt / clippy 失败即 fail 整个 job（不再 `allow_failure: true`）

## 关键前置：contracts-gen 输出格式对齐 rustfmt

最初尝试用 `cargo fmt --all` 修仓库 fmt drift（当时 899 行），但跑完后 `contracts-gen --check` 反而 fail。根因：

- 仓库手维护的 `libs/authz/src/perms.rs` 等生成代码用了「手工格式」（matches! 多分支独立成行、函数体单行），目的是匹配 contracts-gen 生成器输出
- `cargo fmt --all` 按 rustfmt 默认规则把这些手工格式改成 rustfmt 默认格式（matches! 参数独立行、函数体展开多行）
- 生成器输出风格 vs rustfmt 默认风格**不一致**——两者互相冲突

rustfmt 1.96 stable 不支持 `fn_single_line = true`（这是 nightly-only unstable feature），所以无法用 `rustfmt.toml` 锁住单行函数体。

### 修复：改 contracts-gen 生成器输出与 rustfmt 一致

| 文件 | 改动 |
|---|---|
| `contracts/codegen/gen-rust-events.js` | `fn event_type() -> &'static str { "rk" }` → 多行函数体 |
| `contracts/codegen/gen-rust-perms.js` | `matches!(perm, "A" \| "B" ...)` 改为 rustfmt 12 空格缩进格式；末尾加 `\n` |
| `contracts/codegen/gen-rust.js` | 末尾加 `\n`（rustfmt 默认） |
| `libs/authz/src/perms.rs` | 由 `contracts-gen` 重新生成 |
| `libs/common-event/src/events.rs` | 同上 |
| `libs/common-error/src/codes.rs` | 同上 |

效果：
- 生成器输出 = rustfmt 默认输出（cargo fmt --all 不再让生成代码漂移）
- `cargo fmt --all -- --check` 通过（0 drift）
- `contracts-gen --check` 通过（0 drift）

## 实施步骤

### 1. 补 builder 镜像缺失的 rustfmt / clippy components

builder 镜像内置 rustup toolchain `1.96.0-x86_64-unknown-linux-gnu`，但**没有** rustfmt / clippy components。CI 注释（`.gitlab-ci.yml:272-273`）记录 mirror `crates.kudi.repos` 上 `dist/2026-05-28/rustfmt-1.96.0-x86_64-unknown-linux-gnu.tar.xz` 返回 404。本地实测同样证实 404。

**修复路径**：在 `rust_ci` 的 `before_script` 里临时绕过 mirror，让 rustup 走默认上游 `static.rust-lang.org` 装这两个 component（runner 出网可达，本地实测装成功）：

```yaml
before_script:
  - unset RUSTUP_DIST_SERVER RUSTUP_UPDATE_ROOT
  - rustup component add rustfmt clippy
```

这不需要 ops 重建 builder 镜像——后续 ops 真把 component bake 进 builder 镜像后，可去掉这两行（follow-up issue）。

### 2. fmt 修复（前置独立 commit）

仓库当前有 **885 行 fmt drift**（重构后剩余部分），分布在 services/、libs/（除 contracts 生成的 3 个文件外的全部）共 240+ 文件。直接切严格模式会立即阻断 main push。

按 1.2 "One task per commit"：fmt 修复作为独立 commit `chore(fmt): cargo fmt --all`，先 PR 落地，再做 CI 合并改动。

注意：cargo fmt --all 跑完后 `contracts-gen --check` 仍能通过（因为前置的 contracts-gen 输出格式已对齐 rustfmt）。

### 3. CI 合并

合并后的 `rust_ci` job 结构：

```yaml
rust_ci:
  stage: test
  tags: [docker]
  image: $BUILDER_RUST_IMAGE
  variables:
    REDIS_URL: redis://192.168.2.130:6379
  before_script:
    - export RUSTUP_DIST_SERVER=http://crates.kudi.repos
    - export RUSTUP_UPDATE_ROOT=http://crates.kudi.repos/rustup
    # builder 镜像缺 rustfmt/clippy；mirror 404；绕 mirror 走默认 upstream 装
    - unset RUSTUP_DIST_SERVER RUSTUP_UPDATE_ROOT
    - rustup component add rustfmt clippy
  script:
    # 1. fmt check（秒级；先跑尽早 fail，省下后面 clippy/test 的时间）
    - cargo fmt --check --all 2>&1 | tee cargo-fmt.log
    # 2. clippy 编出 workspace 全量（lib + bins + integration tests + benches）；
    #    dump_openapi / test 都复用其 artifact
    - cargo clippy --workspace --all-targets -- -D warnings
    # 3. dump_openapi bin target 在 clippy --all-targets 时已编过，增量 0 成本
    - node scripts/dump-openapi-specs.js
    # 4. openapi drift 单独判定（保留原 spec_generate allow_failure=true 语义）
    - git diff --exit-code -- 'services/*/openapi.yaml' || echo "[rust_ci] openapi drift — non-blocking"
    # 5. test 增量复用 clippy 编过的 lib
    - cargo test --workspace --lib --verbose
  artifacts:
    when: always
    name: "rust-ci-$CI_COMMIT_SHORT_SHA"
    paths:
      - cargo-fmt.log
    expire_in: 1 day
  rules:
    - if: '$CI_COMMIT_BRANCH == "main"'
      when: on_success
    - if: '$CI_PIPELINE_SOURCE == "merge_request_event"'
      when: on_success
    - if: '$CI_COMMIT_TAG =~ /^v\d+\.\d+\.\d+$/'
      when: on_success
  allow_failure: false
```

### 4. 删除的旧 jobs

- `rust_test`（line 98-119）
- `cargo_clippy`（line 242-260）
- `cargo_fmt_check`（line 262-286）
- `spec_generate`（line 160-186）—— node + dump_openapi 步骤迁到 `rust_ci` 内

`spec_generate` 当前的 `dependencies: [spec_check]`（line 173）由 DAG 自然满足：`spec_check` 仍跑在 `test` stage，与 `rust_ci` 并行。

## 收益估算

- **critical path 砍掉 ~10-15 分钟**：合并前 3 次 workspace 完整编译（rust_test / cargo_clippy / spec_generate）→ 合并后 1 次
- **磁盘压力减轻**：去掉 `cargo_clean` workaround（合并后单 job 内 target/ 自然复用，最终大小更小）
- **fmt / clippy 真正生效**：之前 cargo_fmt_check / cargo_clippy 都是 allow_failure=true（builder 缺组件），改完后严格 mode 真正能 fail main push
- **Pipeline 拓扑简化**：test stage 少 3 个并行 job

## 风险 & 缓解

| 风险 | 缓解 |
|---|---|
| runner 出网不通 `static.rust-lang.org` → 装 rustfmt/clippy 失败 | 本地实测可达；CI runner 同网段。若真不通，job 立即 fail（不掩盖） |
| 合并后 clippy 编译失败导致 test 不跑（fail fast） | 设计意图——clippy 没过不该 merge |
| fmt 修复 commit 改动量大（246 文件），PR review 压力大 | spec 已说明这是「前置 commit」；review 时只看 diff 是否纯格式调整（无逻辑改动） |
| 装 rustfmt/clippy 增加 ~30s-2min before_script 时间 | 一次性开销，可忽略；省下的 ~10min 远超 |
| contracts-gen --check 误报 drift（手写文件被粗路径过滤覆盖） | 已观察到 `libs/common-event/src/config.rs`（手写）被 contracts drift 检查路径包含；本次不修（独立 follow-up）；但**不影响**——cargo fmt 后 contracts drift 实际 = 0 |

## 验证

CI gate（按 `.trae/rules/project_rules.md` 1.5）：

```bash
node scripts/check-service-standard.js        # 服务规范
node scripts/check-spec.js                    # OpenAPI / 错误码 / 权限码
node scripts/contracts-gen.js --check         # 契约生成代码与 YAML 一致性（关键）
cargo fmt --all -- --check                    # fmt 0 drift
```

合并 commit 落地后观察：
1. fmt 修复 commit 在 main 上 `rust_ci` job 通过（fmt / clippy / test 全绿）
2. CI 合并 commit 在 main 上 `rust_ci` job 通过
3. MR pipeline 上 `rust_ci` fail-fast 行为正确

## Follow-up

1. ops 把 rustfmt / clippy components bake 进 builder 镜像后，去掉 `rust_ci.before_script` 里 `unset RUSTUP_DIST_SERVER` 和 `rustup component add rustfmt clippy` 两行
2. 收紧 `contracts-gen --check` 的路径过滤（去掉 `libs/common-event/` 等粗路径，改为只检查实际生成的文件）
3. 后续可加 `cache:` 段跨 job 共享 `target/`（等磁盘扩容）
