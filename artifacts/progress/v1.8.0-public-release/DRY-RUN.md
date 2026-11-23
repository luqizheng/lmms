# v1.8.0 公开发布 DRY-RUN 记录

- 执行时间：2026-11-23（Asia/Shanghai）
- 执行环境：`C:\projects\lmms-pr8`（PR #8 分支 `feat/public-release-pipeline` @ `4558aea7`）
- 执行方式：**只读 dry-run**，未登录任何 registry、未推送、未改动 `.image.env`（执行后 `git status` 为空）
- 前置：`node --test packages/cli/test/*.test.js` → **83 passed / 0 failed**

---

## 1. `coder release status`

```text
服务版本（workspace）: 1.8.0
服务版本（解析后）  : 1.8.0
方言 / 镜像源        : pg / zhcoder-docker-registry.com:8000/micro-server
最新 git tag        : v1.8.0
钉版 examples\docker-deploy      : 1.8.0
钉版 examples\docker-deploy-full : 1.8.0
  · api-gateway              1.8.0（继承）
  · token-service            1.8.0（继承）
  · iam-service              1.8.0（继承）
  · notification-service     1.8.0（继承）
  · file-service             1.8.0（继承）
  · audit-service            1.8.0（继承）
  · prue_backend             1.8.0（继承）
  · iam-outbox-publisher     1.8.0（继承）
✓ 版本一致
```

判定：版本一致，无漂移。

---

## 2. `coder release sdk npm --dry-run`

```text
[sdk:npm] dry-run plan:
  - @digua/contracts@0.1.0
  - @digua/http@0.1.0
  - @digua/iam-services@0.1.0
  - @digua/audit-services@0.1.0
  - @digua/file-services@0.1.0
  - @digua/notification-services@0.1.0
  - @digua/prue-services@0.1.0
npm: published 0, skipped 0
```

判定：清单与 PR #8 修复后的 `@digua/` scope 一致；`@digua/cli` 已不在清单中。
**未验证项**：`@digua` organization 是否存在、发布 token 是否有权限（dry-run 不触网鉴权）。

---

## 3. `coder release sdk rust --dry-run`

```text
[sdk:rust] dry-run plan (registry=null):
  - common-error (libs) → digua_common_error@1.6.0
  - common-health (libs) → digua_common_health@1.6.0
  - common-jwt (libs) → digua_common_jwt@1.7.0
  - common-ratelimit (libs) → digua_common_ratelimit@1.6.0
  - common-telemetry (libs) → digua_common_telemetry@1.6.0
  - common-cache (libs) → digua_common_cache@1.6.0
  - common-config (libs) → digua_common_config@1.6.0
  - common-crypto (libs) → digua_common_crypto@1.6.0
  - common-http (libs) → digua_common_http@1.6.0
  - common-log (libs) → digua_common_log@1.6.0
  - common-telemetry-axum (libs) → digua_common_telemetry_axum@1.6.0
  - common-validator (libs) → digua_common_validator@1.6.0
  - jwt-ctx (libs) → digua_jwt_ctx@1.6.0
  - authz (libs) → digua_authz@1.6.0
  - common-db (libs) → digua_common_db@1.6.0
  - common (libs) → digua_common@1.6.0
  - common-event (libs) → digua_common_event@1.6.0
  - digua-http-client (sdks) → digua_http_client@1.7.0
  - audit-service-sdk (sdks) → digua_audit_sdk@1.6.0
  - file-service-sdk (sdks) → digua_file_sdk@1.7.0
  - iam-service-sdk (sdks) → digua_iam_sdk@1.6.0
  - notification-service-sdk (sdks) → digua_notification_sdk@1.6.0
  - token-service-sdk (sdks) → digua_token_sdk@1.6.0
rust: published 0, skipped 0
```

判定：24 个 crate 全部解析成功，`registry=null` 表示走默认 crates.io。
注意：23 个目标版本已在 crates.io 存在（会被 registry 拒绝后跳过），仅 `digua_http_client@1.7.0` 是**首次发布**。

---

## 4. `coder release images --dry-run`

```text
pg dialect: skipping non-DB services (dialect-agnostic): api-gateway, docs-site
version : 1.8.0（来自 services/* crate 版本，dialect=pg）
registry: docker.io/UNCONFIRMED_DOCKERHUB_USER
mode    : local
batch   : dialect-agnostic（无后缀） → api-gateway
batch   : pg（-pg 后缀） → token-service, iam-service, notification-service, file-service, audit-service, prue-admin-backend
将写入的钉版：
  IMAGE_API_GATEWAY=docker.io/UNCONFIRMED_DOCKERHUB_USER/api-gateway:1.8.0
  IMAGE_TOKEN_SERVICE=docker.io/UNCONFIRMED_DOCKERHUB_USER/token-service:1.8.0-pg
  IMAGE_IAM_SERVICE=docker.io/UNCONFIRMED_DOCKERHUB_USER/iam-service:1.8.0-pg
  IMAGE_NOTIFICATION_SERVICE=docker.io/UNCONFIRMED_DOCKERHUB_USER/notification-service:1.8.0-pg
  IMAGE_FILE_SERVICE=docker.io/UNCONFIRMED_DOCKERHUB_USER/file-service:1.8.0-pg
  IMAGE_AUDIT_SERVICE=docker.io/UNCONFIRMED_DOCKERHUB_USER/audit-service:1.8.0-pg
  IMAGE_PRUE_BACKEND=docker.io/UNCONFIRMED_DOCKERHUB_USER/prue-admin-backend:1.8.0-pg
  → examples\docker-deploy\.image.env
  → examples\docker-deploy-full\.image.env
```

判定：镜像计划 7 个，tag 生成规则正确（PG 方言加 `-pg`）。
**未验证项**：`DOCKER_USER_NAME` 真实值、namespace 归属、push 权限。
`UNCONFIRMED_DOCKERHUB_USER` 是占位符，**不代表任何已确认账号**。

---

## 5. Workflow 触发路径静态校验

| 检查 | 结果 |
|---|---|
| `on.workflow_dispatch.inputs.tag` 必填 | 通过 |
| dispatch 路径 checkout 指定 tag | 通过 |
| tag 格式 `^vX.Y.Z$` 校验 | 通过 |
| tag commit 必须是 `origin/main` 祖先（`git merge-base --is-ancestor`） | 通过 |
| 3 个 publish job 均排除 `pull_request` | 通过 |
| 每个 publisher 仅注入自身凭证 | 通过（逐 job `env`） |
| npm/crates 已存在即跳过（幂等） | 通过（由 `sdk-publish` 现有实现保证） |
| Docker 重推幂等 | 覆盖同名 tag，可断点续发 |

---

## 6. 结论

dry-run 全部通过，**发布计划本身可用**。正式发布仍被以下外部条件阻断：

1. `@digua` npm organization 归属与 token 权限未验证；
2. Docker Hub namespace / `DOCKER_USER_NAME` 未确认；
3. `digua_http_client` 首次发布名称未占位；
4. 公开分发缺少负责人书面批准；
5. PR #8 尚未合并（workflow 不在 `main`）。
