# test-case/ 手工验收评估（P2-5）

> **生成时间**：2026-07-30
> **关联**：[pre-v1-todo.md §P2-5](../../pre-v1-todo.md)、[test-case/README.md](../../test-case/README.md)

---

## 1. 评估结论

**20 个 test-case 文件不能也不应该全部迁到 tests/e2e/**。原因：
- 大部分场景需要**观察副作用**（RabbitMQ 队列深度、Redis 滑动窗口、docker logs 内部状态）
- 8 个文件显式依赖 RabbitMQ 管理台 UI / docker logs / 服务重启
- 手工验收是**研发文化**：记录决策意图、变更追溯、操作 SOP，不能纯靠机器断言

**建议策略**：
- 手工验收保留，但加一份 **automated smoke**（拉满置信下限）
- 关键 API 流迁到 `tests/e2e/api/cases/*.yaml`（已有数据驱动框架）
- 副作用观察类永久手工

---

## 2. 分类总览

| 文件 | Phase | 主题 | 可自动化程度 | 建议 |
|------|-------|------|-------------|------|
| `00-setup.md` | — | 起栈 + admin token | N/A | **保留手工**（runbook 性质） |
| `41-notification-welcome.md` | 4.1 | 创建用户 → 欢迎站内信 | **B 部分可迁** | API 流迁 e2e；outbox/RabbitMQ 副作用保留手工 |
| `42-login-failed-risk.md` | 4.2 | 登录失败 + 风控窗口 | **B 部分可迁** | API 流可迁；Redis 窗口 / docker logs 副作用保留 |
| `43-announcement-ws.md` | 4.3 | 公告 WS 广播（多副本） | **C 保持手工** | WS 多副本广播需观察 management UI |
| `43-fu2-recipient-resolver.md` | 4.3-fu2 | 收件人解析 | A | **建议迁**（纯 API 校验） |
| `44-fu1-user-roles.md` | 4.4-fu1 | 用户角色权限 | A | **建议迁**（纯 SQL + API） |
| `44-fu2-actor-id.md` | 4.4-fu2 | 操作者 ID 透传 | B | 部分可迁（API 头字段） |
| `44-permission-epoch.md` | 4.4 | 权限变更 → 令牌失效 | **C 保持手工** | 跨服务 Redis 状态 + docker logs |
| `51-mtls-mesh.md` | 5.1 | mTLS 服务网格 | **C 保持手工** | 证书配置 + docker logs |
| `51-resilience-breaker.md` | 5.1 | 熔断器 | B | 部分可迁；触发熔断需手工停服务 |
| `5b-fu3-event-sync.md` | 5B-fu3 | 事件同步 | **C 保持手工** | RabbitMQ 队列深度观察 |
| `61-trace-id-injection.md` | 6.1 | trace_id 注入 | B | 部分可迁；X-Trace-Id 校验可自动化 |
| `62-otel-cross-service-chain.md` | 6.2 | OTel 跨服务链路 | **C 保持手工** | 需在 Jaeger UI 查 trace |
| `63-consumer-error-classes.md` | 6.3 | consumer 错误分级 | **C 保持手工** | 需观察 docker logs 分级日志 |
| `64-outbox-archive.md` | 6.4 | outbox 归档 | B | 部分可迁（检查 archive 表） |
| `65-config-service-dogfood.md` | 6.5 | config-service 自吃狗粮 | A | **建议迁**（纯 API CRUD） |
| `81-tenant-registration.md` | 8.1 | 租户注册 | A | **建议迁**（纯 API） |
| `82-tenant-business-tables.md` | 8.2 | 租户业务表 | A | **建议迁**（纯 SQL + API） |
| `83-cross-tenant.md` | 8.3 | 跨租户 | A | **建议迁**（pure API negative tests） |

---

## 3. 推荐迁移清单（A 类，共 7 个）

按 ROI 从高到低：

1. **`65-config-service-dogfood.md`**（Phase 6.5 dogfood）
   - 16 个 curl/mysqlq 命令
   - 纯 API CRUD，无副作用
   - 迁到 `tests/e2e/api/cases/config-dogfood.yaml`

2. **`82-tenant-business-tables.md`**（Phase 8.2）
   - 19 个命令
   - 纯 SQL 校验 + API CRUD
   - 迁到 `tests/e2e/api/cases/tenant-business.yaml`

3. **`83-cross-tenant.md`**（Phase 8.3）
   - 19 个命令
   - 跨租户隔离 negative tests（平台租户访问其他租户数据应 403）
   - 迁到 `tests/e2e/api/cases/cross-tenant.yaml`（安全关键）

4. **`81-tenant-registration.md`**（Phase 8.1）
   - 5 个 curl 命令
   - 纯 API 注册流程
   - 迁到 `tests/e2e/api/cases/tenant-registration.yaml`

5. **`44-fu1-user-roles.md`**（Phase 4.4-fu1）
   - 8 个命令
   - 纯 SQL 校验角色权限映射
   - 迁到 `tests/e2e/api/cases/user-roles.yaml`

6. **`43-fu2-recipient-resolver.md`**（Phase 4.3-fu2）
   - 命令数较少
   - 纯 API 校验（收件人解析）
   - 迁到 `tests/e2e/api/cases/recipient-resolver.yaml`

7. **`44-fu2-actor-id.md`**（Phase 4.4-fu2）
   - B 类边缘
   - 可迁操作者 ID 头字段校验部分
   - 迁到 `tests/e2e/api/cases/actor-id.yaml`

---

## 4. 永久手工场景（C 类，9 个）

保留 runbook 价值，**不迁**：

- `43-announcement-ws.md` — WS 多副本广播需 management UI
- `44-permission-epoch.md` — 跨服务 Redis 双写 + docker logs
- `51-mtls-mesh.md` — mTLS 证书 + docker logs
- `5b-fu3-event-sync.md` — RabbitMQ 队列深度
- `62-otel-cross-service-chain.md` — Jaeger UI trace 查询
- `63-consumer-error-classes.md` — docker logs 分级
- `00-setup.md` — runbook 性质

**B 类 5 个**保留为手工（部分步骤可迁但人工观察成本低于自动化收益）：
- `41-notification-welcome.md` / `42-login-failed-risk.md` / `51-resilience-breaker.md` / `61-trace-id-injection.md` / `64-outbox-archive.md`

---

## 5. CI 处理建议

**保留手工 + CI 烟测**：在 `.gitlab-ci.yml` 加 `e2e_testcase_smoke` job，迁完 §3 的 7 个文件后跑：

```yaml
e2e_testcase_smoke:
  stage: test
  tags: [docker]
  image: $BASE_REGISTRY/node:20
  before_script:
    - docker compose -f infrastructure/docker-compose.yml up -d
  script:
    - pnpm -C tests/e2e install
    - pnpm -C tests/e2e test:api  # 跑全量 API 用例，包含迁过去的 7 个
  rules:
    - changes:
        - test-case/**
        - tests/e2e/api/cases/**
      when: on_success
    - when: never
```

**不阻断主流程**：`allow_failure: true`（手工验收是 SOP，自动化只补下限）。

---

## 6. 评估方法说明

**判定标准**（按步骤表内容）：

| 信号 | 含义 | 分类 |
|------|------|------|
| 仅 `curl` / `mysqlq` 查询 | 纯断言 | A |
| 含 `docker logs` / `docker exec` 检查 | 内部状态观察 | C |
| 含 RabbitMQ 管理台 / 队列深度 | 副作用观察 | C |
| 含"重启" / "停容器"步骤 | 混沌测试 | B/C |
| 含 mTLS / 证书命令 | 配置类 | C |
| 含 JSON 解析 / 字段断言 | 可断言 | A/B |

---

## 7. 1.0 准入评估

✅ **通过**。test-case/ 全部保留手工价值，无阻塞项。

**1.0 后跟进**：
1. 选 2-3 个 A 类（推荐 65/82/83）作为模板迁过去
2. 验证 `tests/e2e/api/cases/*.yaml` 数据驱动框架能承载这些场景
3. 跑通后用 `e2e_testcase_smoke` job 卡 CI 下限