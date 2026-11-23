# v1.8.0 公开制品补发清单（INVENTORY）

- 生成时间：2026-11-23（Asia/Shanghai）
- 目标：在**不执行任何发布动作**的前提下，逐项确认 v1.8.0 的公开制品现状、权限与补发可行性
- 数据来源：crates.io API / registry.npmjs.org / hub.docker.com API / GitHub REST API / 本地 `coder release --dry-run`
- 结论：**当前不具备正式补发条件**（见文末「放行判定」）

---

## 0. 版本基线

| 项 | 值 | 来源 |
|---|---|---|
| 平台版本（workspace） | `1.8.0` | 根 `Cargo.toml` `[workspace.package].version` |
| `v1.8.0` tag 指向 | `5b675086`（= `main` 当前 tip） | GitHub tags API |
| GitHub Release | 存在，非 draft、非 prerelease，**0 assets** | Releases API |
| Rust crate 版本 | libs `1.6.0`（`digua_common_consul`/`digua_common_jwt` 为 `1.7.0`）；sdk `1.6.0`/`1.7.0` | 本地 manifest |
| npm 包版本 | **全部 `0.1.0`** | 7 个 `package.json` |
| 镜像 tag | `1.8.0`（无 DB）/ `1.8.0-pg`（PG 方言） | `.image.env` + dry-run |

> 注意：Rust / npm 制品版本与平台版本**解耦**。v1.8.0 补发不会产生 `1.8.0` 版本的 crate / npm 包。

---

## 1. Rust crates（目标平台：crates.io）

所有权：抽查 `digua_common_error`、`digua_common_consul`、`digua_jwt_ctx`、`digua_audit_sdk`，owners 均为 `luqizheng` → **本项目所有，具备发布权限**。

| crate | 目标版本 | 已存在 | 允许补发 | 命令 | 风险 |
|---|---|---|---|---|---|
| digua_common_error | 1.6.0 | 是 | 否（版本已存在，会跳过） | `cargo publish -p digua_common_error` | 无实际产出 |
| digua_common_health | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common_jwt | 1.7.0 | 是 | 否 | 同上 | 同上 |
| digua_common_ratelimit | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common_telemetry | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common_cache | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common_config | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common_crypto | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common_http | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common_log | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common_telemetry_axum | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common_validator | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_jwt_ctx | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_authz | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common_db | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common_event | 1.6.0 | 是 | 否 | 同上 | 同上 |
| digua_common_consul | 1.7.0 | 是 | 否 | 同上 | 同上 |
| **digua_http_client** | **1.7.0** | **否（名称当前可用）** | **是（首次发布）** | `cargo publish -p digua_http_client` | **名称尚未占位，存在被抢注风险；首次发布不可撤回** |
| digua_audit_sdk | 1.6.0 | 是 | 否 | 同上 | — |
| digua_file_sdk | 1.7.0 | 是 | 否 | 同上 | — |
| digua_iam_sdk | 1.6.0 | 是 | 否 | 同上 | — |
| digua_notification_sdk | 1.6.0 | 是 | 否 | 同上 | — |
| digua_token_sdk | 1.6.0 | 是 | 否 | 同上 | — |

统一入口：`node packages/cli/bin/coder.js release sdk rust --push`

---

## 2. npm 包（目标平台：npmjs.org）

| 名称 | 目标版本 | 已存在 | 所有者/权限 | 允许补发 | 命令 | 风险 |
|---|---|---|---|---|---|---|
| @digua/contracts | 0.1.0 | 否（404） | `@digua` scope 归属**未验证** | 待确认 | `npm publish` | scope 若未注册/无权限 → 403 |
| @digua/http | 0.1.0 | 否（404） | 同上 | 待确认 | 同上 | 同上 |
| @digua/iam-services | 0.1.0 | 否（404） | 同上 | 待确认 | 同上 | 同上 |
| @digua/audit-services | 0.1.0 | 否（404） | 同上 | 待确认 | 同上 | 同上 |
| @digua/file-services | 0.1.0 | 否（404） | 同上 | 待确认 | 同上 | 同上 |
| @digua/notification-services | 0.1.0 | 否（404） | 同上 | 待确认 | 同上 | 同上 |
| @digua/prue-services | 0.1.0 | 否（404） | 同上 | 待确认 | 同上 | 同上 |

补充事实：

- `npm search scope:digua` 返回 **0 个公开包** → scope 下暂无任何已发布包。
- registry 的 org 查询端点不公开，**无法匿名验证 `@digua` organization 是否存在、是否归本项目所有**。
- 历史未 scoped 名称现状（说明为何必须统一加 scope）：
  - `file-services` → **已被他人占用**（`gopinathkaliappan`，0.0.2）
  - `notification-services` → **已被他人占用**（`arachnomesh`，1.2.0）
  - `iam-services` → 存在空壳记录但无任何版本（不可用于新发布）
  - `audit-services`、`prue-services` → 404，可用
- 7 个包 license 已由 `UNLICENSED` 改为 `Apache-2.0`（PR #8 分支 `4558aea7`）。

统一入口：`node packages/cli/bin/coder.js release sdk npm --push`

---

## 3. Docker 镜像（目标平台：Docker Hub）

| 镜像名 | 目标 tag | 已存在 | 所有者/权限 | 允许推送 | 命令 | 风险 |
|---|---|---|---|---|---|---|
| api-gateway | 1.8.0 | 未知 | **namespace 未确认**（`DOCKER_USER_NAME` secret 不可见） | 待确认 | `coder release images --push --registry docker.io --repo <ns>` | namespace 错误 → 推送至错误空间 |
| token-service | 1.8.0-pg | 未知 | 同上 | 待确认 | 同上 | 同上 |
| iam-service | 1.8.0-pg | 未知 | 同上 | 待确认 | 同上 | 同上 |
| notification-service | 1.8.0-pg | 未知 | 同上 | 待确认 | 同上 | 同上 |
| file-service | 1.8.0-pg | 未知 | 同上 | 待确认 | 同上 | 同上 |
| audit-service | 1.8.0-pg | 未知 | 同上 | 待确认 | 同上 | 同上 |
| prue-admin-backend | 1.8.0-pg | 未知 | 同上 | 待确认 | 同上 | 同上 |

补充事实：

- 现有 `.image.env` 钉的是内网 registry `zhcoder-docker-registry.com:8000/micro-server/*`，**不是 Docker Hub**。
- 查询 `luqizheng`、`digua`、`lmms` 三个候选 namespace，均返回 0 个公开仓库 → 无法证实任何候选归属本项目。
- Docker Hub 同名 tag 重推会覆盖；**一旦公开，镜像内容不可撤回**。

---

## 4. 授权 / 许可证确认

| 项 | 状态 |
|---|---|
| 仓库根 `LICENSE` | PR #8 分支已新增 Apache-2.0 |
| npm 公开包 license | 已统一 `Apache-2.0`（不再使用 `UNLICENSED`） |
| Rust crate license | 全部 crate 已声明 `license`（Apache-2.0） |
| 法务/owner 书面批准公开发布 | **无记录，需项目负责人确认** |

---

## 5. 放行判定

**当前不允许正式补发。** 阻断项：

1. `@digua` npm organization 归属与发布 token 权限未验证。
2. Docker Hub 目标 namespace（`DOCKER_USER_NAME`）未确认，无法核对仓库归属。
3. `digua_http_client@1.7.0` 在 crates.io 尚不存在且名称未占位（首次发布不可撤回）。
4. 公开发布的法务批准无记录。
5. PR #8 尚未合并（发布 workflow 不在 `main` 上）。

非阻断但需知悉：

- v1.8.0 补发对 crates.io 的 23 个已发布 crate 是**空操作**（版本已存在会跳过）。
- 所有 npm 包目标版本为 `0.1.0`，与平台 `1.8.0` 不匹配（版本线独立，属设计选择）。
