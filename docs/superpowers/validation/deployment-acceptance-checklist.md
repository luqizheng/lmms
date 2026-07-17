# 系统部署验收 Checklist

> 适用范围：在合并到 `master` 并触发自动部署**之前**，由人工执行最后一次静态 + 动态核验。
> 维护：每完成一个 Phase 加一节；既有 `test-case/41-44` 场景继续作为功能深挖入口。

## 0. 使用约定

- 每张步骤表 5 列：`步骤 / 操作 / 期望 / 实测 / P·F`，空格填实测结果、末尾勾 P 或 F。
- 任何一步 ❌ → 阻塞部署；先修，再回此处重跑。
- 时间估算：纯 CI 复跑 5 min；本地全栈起来 + smoke 15 min；含业务深挖 2-3 h。
- 与既有测试约定一致：先做 `test-case/00-setup.md` 拿 `$BASE` / `$TOKEN`，再做本表。

---

## 1. 静态合规（push 前必过）

| # | 命令 | 期望 | 实测 | P/F |
|---|------|------|------|-----|
| 1.1 | `git status --short` | 仅允许 `infrastructure/base-images.json` 时间戳漂移或空 | | |
| 1.2 | `node scripts/check-service-standard.js` | 末尾打印 `✅ 服务规范检查通过` | | |
| 1.3 | `node scripts/check-spec.js` | 末尾 `✅ 所有契约规范检查通过`（summary 缺失为告警非阻塞） | | |
| 1.4 | `node scripts/contracts-gen.js --check` | 末尾 `✅ Contracts in sync` | | |
| 1.5 | `node scripts/check-db-naming.js` | `PASS: no bare table names found` | | |
| 1.6 | `scripts/dump-openapi-specs.sh && git diff --exit-code -- 'services/*/openapi.yaml'` | 无 diff（exit 0） | | |
| 1.7 | `cargo check --workspace --target x86_64-unknown-linux-musl` | 编译通过，0 error | | |
| 1.8 | `cargo clippy --workspace -- -D warnings` | 0 warning | | |

> **判定**：1.1 ~ 1.8 全 P → 合规通过；任一 F → 修后再 push。

---

## 2. CI 流水线（push 到非 master 分支触发）

| # | 检查 | 期望 | 实测 | P/F |
|---|------|------|------|-----|
| 2.1 | `rust_test` job | ✅ passed（含 `cargo test --workspace --lib`） | | |
| 2.2 | `service_standard_check` | ✅ passed | | |
| 2.3 | `spec_check` | ✅ passed | | |
| 2.4 | `contracts_check` | ✅ passed | | |
| 2.5 | `spec_generate`（OpenAPI drift） | ✅ passed（`git diff --exit-code` 退出 0） | | |
| 2.6 | `build_api_gateway` / `build_token_service` / ... 9 个 build 任务 | 全部 ✅ passed，镜像推 `:sha` + `:latest` | | |
| 2.7 | `e2e_api_test`（smoke + 全量 API） | ✅ passed | | |
| 2.8 | `e2e_ui_test`（可选） | ✅ passed 或标记 skip | | |

> **判定**：test + build 阶段全 P → 可合并 master 触发 deploy。

---

## 3. 本地全栈起停

> 这一节假设你在能跑 docker 的开发机；当前 AI 沙箱因 docker daemon 不可用无法实跑。

| # | 命令 | 期望 | 实测 | P/F |
|---|------|------|------|-----|
| 3.1 | `docker info` | `Server Version` 非空，无 `I/O error` | | |
| 3.2 | `docker compose -f infrastructure/docker-compose.yml up -d` | 全部容器 Exit 0 | | |
| 3.3 | `docker compose -f infrastructure/docker-compose.yml ps` | consul/redis/mysql/`*healthy`；9 个 service `running` | | |
| 3.4 | `curl -s localhost:8500/v1/status/leader` | 200 + 有效 leader IP | | |
| 3.5 | `curl -s localhost:3000/health` | 200 + `{"status":"ok"}` | | |
| 3.6 | `curl -s localhost:3000/.well-known/jwks.json` | 200 + 含 `keys` 数组 | | |
| 3.7 | 7 个业务服务 `/health`（经 gateway `v1/{svc}/health`）| 200 全过 | | |
| 3.8 | `curl -s localhost:15672`（RabbitMQ 管理台，guest/guest） | 能登入 | | |
| 3.9 | `node scripts/check-dev-env.js` | 末尾 exit 0，全部 `[32mOK[0m` | | |
| 3.10 | `docker compose -f infrastructure/docker-compose.yml down` | 全部 Exit 0 | | |

> **判定**：3.1 ~ 3.10 全 P → 本地基础设施可用；进入 §4。

---

## 4. Rust 测试（容器内）

| # | 命令 | 期望 | 实测 | P/F |
|---|------|------|------|-----|
| 4.1 | `node scripts/run-tests-docker.js` | 末尾 `pass : N`，无 `fail` | | |
| 4.2 | `node scripts/run-tests-docker.js iam-service` | iam 单服务全过 | | |
| 4.3 | `node scripts/run-tests-docker.js token-service` | token 全过 | | |
| 4.4 | `node scripts/run-tests-docker.js notification-service` | notification 全过（含 Phase 4.3-fu3 fanout 回归 48/48） | | |
| 4.5 | `node scripts/run-tests-docker.js audit-service` | audit 全过（含 Phase 6-A T2 OTel 测试） | | |

> **判定**：4.1 全过，或单服务逐个 4.2 ~ 4.5 全过 → Rust 端通过；进入 §5。

---

## 5. E2E（数据驱动 API）

> 前置：`pnpm -C tests/e2e install`

| # | 命令 | 期望 | 实测 | P/F |
|---|------|------|------|-----|
| 5.1 | `pnpm -C tests/e2e test:smoke` | 8 API + 2 UI 用例全过（2 min） | | |
| 5.2 | `pnpm -C tests/e2e test:api` | 5 min 全量 API 通过 | | |
| 5.3 | `pnpm test:e2e:auth` | 登录 / 注册 / token 刷新通过 | | |
| 5.4 | `pnpm test:e2e:users` / `:tokens` / `:roles` / `:permissions` | 各自通过 | | |
| 5.5 | `pnpm test:e2e:audit` / `:config` / `:file` / `:notification` | 各自通过 | | |
| 5.6 | `pnpm test:e2e:menu-configs` / `:iam-orgs` / `:gateway` | 各自通过 | | |

> **判定**：5.1 ~ 5.6 全过 → API 层通过；进入 §6。

---

## 6. 业务场景（手工 / test-case/）

> 前置：完成 `test-case/00-setup.md` 拿 `$BASE` `$TOKEN`。

| # | 场景 | 文件 | 期望 | 实测 | P/F |
|---|------|------|------|------|-----|
| 6.1 | 新建用户 → 欢迎站内信（事件驱动） | `test-case/41-notification-welcome.md` | 收件箱有欢迎公告 | | |
| 6.2 | 登录失败事件 + 风控滑动窗口 | `test-case/42-login-failed-risk.md` | 失败 N 次后告警入审计 | | |
| 6.3 | 公告发布 → 事件驱动 WebSocket 广播（多副本） | `test-case/43-announcement-ws.md` | WS 客户端收到 push | | |
| 6.4 | 权限变更 → 令牌失效（perm-epoch） | `test-case/44-permission-epoch.md` | 旧 token 401，新 token 200 | | |
| 6.5 | mTLS 服务网格（Consul Connect） | `test-case/51-mtls-mesh.md` | allow/deny 意图生效 | | |
| 6.6 | OTel 跨服务 trace 链 | `test-case/62-otel-cross-service-chain.md` | Jaeger 单 trace 串联所有 span | | |
| 6.7 | consumer 错误分类（Transient/Permanent + DLQ 阈值 3） | `test-case/63-consumer-error-classes.md` | 重试 N 次后入 DLQ | | |
| 6.8 | outbox archive 行为 | `test-case/64-outbox-archive.md` | 已发布行归档到 `*_archive` 表 | | |

> **判定**：6.1 ~ 6.4 是 Phase 4 主路径必须；6.5 ~ 6.8 是 Phase 5/6 验收项。建议 6.1 ~ 6.4 必跑，6.5 ~ 6.8 视 release 节奏选跑。

---

## 7. 可观测性

| # | 命令 / 操作 | 期望 | 实测 | P/F |
|---|------|------|------|-----|
| 7.1 | `bash scripts/start-observability.sh` | Prometheus :9090 + Grafana :3001 + Jaeger :16686 + OTel :4317 起来 | | |
| 7.2 | Grafana → "Gateway Overview" dashboard | 7 个 service 的 QPS / latency / error rate 都有曲线 | | |
| 7.3 | Jaeger → 选 `api-gateway=*` 任意 trace | 能看到下游 token-service / iam-service / audit-service span | | |
| 7.4 | `curl localhost:3000/metrics` | Prometheus 文本格式，含 `http_requests_total` | | |
| 7.5 | 7 个业务服务 `/metrics`（经 gateway）| 同上，每个服务都有 | | |

> **判定**：7.1 ~ 7.5 全 P → 可观测性就绪。

---

## 8. 生产部署预演（仅 master 分支触发）

> 这一节不在本地跑，看 GitLab `deploy` job 自动执行。

| # | 检查 | 期望 | 实测 | P/F |
|---|------|------|------|-----|
| 8.1 | GitLab CI Variables 完整 | `SSH_PRIVATE_KEY` / `DEPLOY_SERVER` / `DEPLOY_USER` / `DEPLOY_PASSWORD` / `DOCKER_REGISTRY` / `DOCKER_REPO` 全部存在且 `Protected` | | |
| 8.2 | 部署机 `192.168.2.128` 预检 | `docker info` 正常；`/home/kudi/coder-micro-services` 目录可写；磁盘 > 10 GB | | |
| 8.3 | merge 到 master → `deploy` job | `docker-compose pull && up -d` 全过；退出 0 | | |
| 8.4 | 部署后 smoke | `curl 192.168.2.128:3000/health` 200；任选 `v1/iam/health` 200 | | |
| 8.5 | 部署后 image tag | `docker images | grep micro-server` 见 `:$CI_COMMIT_SHORT_SHA` + `:latest` 双 tag | | |
| 8.6 | 部署后 Prometheus 抓取 | 4 个 OTel-enabled service 上报 trace | | |

> **判定**：8.1 ~ 8.6 全 P → 生产部署完成。

---

## 9. 验收签字

| 角色 | 姓名 | 日期 | 备注 |
|------|------|------|------|
| 执行人 | | | |
| 复核人 | | | |
| 批准人 | | | |

---

## 附录 A：故障排查速查

| 现象 | 命令 | 处置 |
|------|------|------|
| MySQL 找不到 | `docker ps -a | grep mysql` | 容器名带 `coder-micro-services-` 前缀是正常的；按名引用即可 |
| cargo 链接 fiat_25519 报错 | `grep aws_lc_rs Cargo.toml` | 必须保留 `aws_lc_rs`；切回 `ring` 会触发 MSVC LNK2019 |
| OpenAPI drift CI 失败 | `scripts/dump-openapi-specs.sh && git add services/*/openapi.yaml && git commit` | commit 漂移文件 |
| API 返回 429 | `cat infrastructure/config/api-gateway/rate-limits.toml` | 调高 `requests_per_second` 或加白名单 |
| trace 不在 Jaeger | `echo $OTEL_EXPORTER_OTLP_ENDPOINT` | 应为 `http://otel-collector:4317` |
| RabbitMQ 拔线后 user 创建卡住 | `docker logs iam-outbox-publisher` | publisher 10s 内追上（Phase 4 C1 修复） |

## 附录 B：相关文档

- `AGENTS.md` — 项目 AI 协作指南
- `docs-site/ai-context.md` — 服务地图、ADR 索引、pitfall 列表
- `test-case/` — 手工场景步骤表
- `tests/e2e/` — 自动化 API + UI 用例
- `docs/superpowers/specs/` — 各 Phase 设计文档
- `docs/superpowers/plans/` — 各 Phase 实施计划
- `docs-site/runbooks/secret-rotation.md` — 密钥轮换 runbook