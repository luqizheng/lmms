# 04. 测试

> 跑测、读测试、新增 E2E 用例。

---

## 4.1 测试金字塔

```
┌────────────────────────────────────┐
│  E2E (Playwright UI + Vitest API)  │  ← 真实服务在 docker 中
│  - smoke：2 min（8 API + 2 UI）    │
│  - full API：约 5 min              │
│  - full UI：约 10 min              │
├────────────────────────────────────┤
│  Rust 集成测试                     │  ← cargo test --test <name>
│  - sqlx / mockall / testcontainers │
├────────────────────────────────────┤
│  Rust 单元测试                     │  ← cargo test --lib
│  - 纯业务逻辑，无 I/O              │
└────────────────────────────────────┘
```

越往下越快、越窄、越纯；越往上越慢、越接近真实场景。CI 默认全跑，开发循环里**至少跑冒烟**。

---

## 4.2 跑某个服务的全部测试

```bash
node scripts/run-tests-docker.js iam-service
```

等价于：

```bash
docker run --rm --network host \
  -v $(pwd):/app \
  -v coder-cargo-target:/app/target \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -w /app \
  zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl \
  cargo test --target x86_64-unknown-linux-musl -p iam-service
```

> **为什么走容器**：本机常缺 `cc` 链接器与 musl target。容器挂载 `coder-cargo-target` volume 复用增量编译。

---

## 4.3 跑单个测试

```bash
# Rust 集成测试（按 name 过滤）
cargo test --test integration_test -- --nocapture test_login

# E2E 冒烟子集
cd tests/e2e && E2E_SMOKE=1 pnpm test:api
```

---

## 4.4 E2E 分层：smoke vs api vs ui

| 层级 | 时间 | 用例数 | 适用场景 |
|------|------|--------|----------|
| **smoke** | 2 min | 8 API + 2 UI | pre-commit、开发循环 |
| **api** | 5 min | 全量 API（50+） | pre-deploy、CI |
| **ui** | 10 min | 全量 UI（20+） | pre-deploy、CI |

在 YAML 用例文件里给关键 happy-path 加 `smoke: true`，它就会被冒烟套件自动收录。

---

## 4.5 新增 E2E 用例（数据驱动）

`tests/e2e/api/cases/iam-users.yaml`：

```yaml
- caseId: IAM-USER-042
  desc: 新建用户用例
  method: POST
  path: /v1/iam/users
  body:
    username: new${rand}
    password: P@ssw0rd!
  expectStatus: 200
  expect:
    - $.id exists
  smoke: true   # 纳入冒烟子集
```

完整字段（断言表达式、变量插值、依赖用例）见 [`docs-site/guides/testing.md`](../guides/testing.md)。

---

## 4.6 CI 中跑什么

GitLab CI `test` 阶段：

- `service_standard_check`
- `spec_check`
- `contracts_check`
- `spec_generate`（产出 openapi.yaml，触发 drift 检测）
- `e2e_api_test`
- `e2e_ui_test`
- `rust_test`（workspace 全量）

详见 [06 部署](./06-deployment.md)。

---

## 4.7 本地复刻 CI

```bash
node scripts/check-service-standard.js && \
  node scripts/check-spec.js && \
  node scripts/contracts-gen.js --check && \
  node scripts/dump-openapi-specs.js && \
  git diff --exit-code -- 'services/*/openapi.yaml' && \
  node scripts/run-tests-docker.js iam-service
```

或在 shell rc 里加 alias：

```bash
alias cms-ci='node scripts/check-service-standard.js && node scripts/check-spec.js && node scripts/contracts-gen.js --check'
```

---

## 4.8 写测试的最佳实践（ponytail 提示）

- **断言越具体越好**：不要 `assert!(result.is_ok())`，要 `assert_eq!(result.unwrap().status, 200)`。
- **不要 sleep**：用轮询 + 超时代替固定等待。
- **每个测试自己造数据**：不依赖其它用例留下的状态。
- **命名要能搜**：`test_login_with_invalid_password_returns_401` 比 `test_login_1` 好。
