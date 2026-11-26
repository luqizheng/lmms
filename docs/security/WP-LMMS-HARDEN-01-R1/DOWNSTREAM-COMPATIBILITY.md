# WP-LMMS-HARDEN-01-R1 · 下游职责与兼容性检查

- 审计对象：`ai-pm`（`docs/wp-lmms-delegation-01` @ `6f101ce9`，master 基线 `6f317176`）、`smark-artifact-service`（主树 `8fd5173` / r1 树 `b40bc9d`）、`smark-sem`（`main` @ `344a9da`，只读克隆）
- LMMS 基线：`main` @ `92691f33`；修复分支 `fix/wp-lmms-harden-01-r1`
- 性质：只读检查（本工作包不修改其他仓库）

---

## 1. 任务书七问逐答

### ① 当前下游是否实际使用 `/tokens/resource`？

**否——三个下游仓库均无任何代码调用**【代码证实】。

| 仓库 | `/tokens/resource` 命中 | 性质 |
|---|---|---|
| ai-pm | `infrastructure/full-dev/config/api-gateway/routes/token.router.toml:20-23`（`public = true`） | **部署配置引用**（网关透传路由，非业务调用）+ 文档（WP-01 集成说明："`/tokens/resource` 不可用…不要基于它做集成"） |
| smark-artifact-service | `crates/artifact-application/src/port/auth.rs:4`、`crates/artifact-infrastructure/src/delegation/mod.rs:10` 注释 | **明文声明不消费**（"未挂载认证 middleware"是 AS 侧不用它的理由）；`ART-02-SECURITY-STATUS.md:9`："不消费该 endpoint。仓库内无任何对 lmms resource-token 的调用代码" |
| smark-sem | 0 命中 | 纯文档仓库，仅部署审计历史提及 token-service:8082 拓扑 |

唯一封装了 `issueResource(...)` 的代码是 lmms 仓库**内部**的 Java SDK（`sdk/java/digua-token-client-sdk`、`digua-token-user-sdk`），三个下游仓库均不依赖它们。历史 pm-auth/pm-sdk/pm-attachment 模块（已迁出 lmms）也从未调用该端点。

**因此本次修复（加认证）不破坏任何现有调用方。**

### ② 如有使用，具体调用场景？

不存在调用方，场景不适用。下游实际使用的 LMMS 能力是：
- ai-pm：`POST /tokens/service-jwt`（经 `digua_common_jwt::ServiceJwtCache`，bootstrap token 换 per-tenant `aud=file-service` service-jwt）+ file-service 上传/下载/presign + `POST /tokens/user-jwt`（iam 内部登录链）+ JWKS 验签 LMMS 用户 JWT。
- AS：不调用 LMMS 任何 API；其 JWKS 信任根指向 **AI-PM**（`ARTIFACT_API_DELEGATION_ISSUERS`，aud 恒 `artifact-service`）。
- SEM：无任何 token 使用。

### ③ 下游是否把业务授权错误交给 LMMS？

**没有**【代码证实】：
- ai-pm 自持业务授权：租户/角色取库不取 token、权限码直比、拒绝 `roles=["service"]` 与 `ct=true` token（`apps/pm-api/src/auth.rs:20-21,136-152,329-337`）；文件链是"业务授权决策在前，再以受限 service-jwt 代理调用"。
- AS 自持委托授权（`Delegation::require` + actions + V10 绑定，验证器全在 AS 内部）。
- file-service 明确不判断业务归属（`contracts/http-conventions.md:62-71`），user_id 由调用方负责。

### ④ 是否存在用户/服务/Resource/Delegation Token 混用？

**无实际混用**【代码证实】。token 类型以 `purpose` 权威分类：ai-pm 把 `service|resource` 与 `ct=true` 一律拒于业务面；AS 只接受 `purpose=delegation` 且 aud=artifact-service；LMMS 内部 service-jwt 与用户 JWT 以 purpose/sub/aud/roles 四要素区分。唯一历史例外是已迁出的 Java PM 在 dev profile 使用静态 dev-bypass-token（现不在任何相关仓库）。

### ⑤ 是否需要统一 Token claims 或 JWKS 契约？

**建议统一，但不由 LMMS 单方面改动**（跨仓库契约决策）：
- 现状差异：LMMS 用户 JWT aud 固定 `coder-micro-services`、OAuth token 无 aud；AS 委托链要求 `aud=artifact-service` + `purpose=delegation` + tenant_id(UUID)+tid 双映射——AI-PM 集成说明已把"LMMS 用户 JWT aud 硬编码"列为委托链阻断项 P0-02。
- JWKS 契约本身健康（RS256、kid 严格、active/retiring 发布、轮换状态机），ai-pm 与 AS 的 JWKS 消费实现兼容；无需改动。
- 建议：跨仓库 token 契约（aud 语义、purpose 枚举、claims 最小集）在 WP-DOC-RATIFY 类裁决包中统一决策（见 RECLASSIFICATION 报告 §G 工作包 #1）。

### ⑥ 是否需要修改下游文档或 SDK？

- **LMMS 侧（本包已改）**：`docs-site/guides/token-service-overview.md`、`services/token-service/{AGENTS,README,FEATURES}.md` 中 `/tokens/resource` 的鉴权描述已从 public 改为 service-jwt。
- **Java SDK（lmms 仓库内）**：`issueResource` 无需签名变更——`SdkClient` 本就支持附带 bearer；但调用方必须配置持有 `aud=token-service` service-jwt 才能成功（行为变化已记录于 RESOURCE-TOKEN-ANALYSIS §8）。可在 SDK 文档补一句准入要求（非阻塞，未在本包改动范围）。
- **ai-pm 侧（下游行动项，本包不改）**：见 §3。

### ⑦ 是否存在不兼容风险？

**修复本身：无**（零下游调用方）。详见 §2 影响矩阵。间接风险只有"未来有业务系统按旧文档尝试匿名调用"——已被文档同步阻断。

## 2. 修复后的兼容性影响矩阵

| 调用方 | 修复前 | 修复后 | 影响 |
|---|---|---|---|
| 匿名调用（任意公网客户端） | 200（获得任意 claims 签名 JWT） | **401**（网关 401 / token-service 401 双层） | 预期拦截对象 |
| 持用户 JWT 的调用 | 200 | 网关放行（有 tid 即可）→ token-service 401（aud 不符） | 预期拦截对象 |
| 持 `aud=token-service` service-jwt 的平台服务 | 200 | 200（不变） | 无影响 |
| 持其他 aud service-jwt | 200 | 401 | 预期收紧 |
| ai-pm 业务代码 | 不调用 | 不调用 | 无影响 |
| Java SDK `issueResource` | 匿名可用 | 须附 service-jwt bearer | 行为收紧，SDK 兼容（bearer 透传） |

## 3. 下游行动项清单（本包不执行，需下游工作包承接）

| # | 仓库 | 行动项 | 依据 |
|---|---|---|---|
| D-1 | **ai-pm** | `infrastructure/full-dev/config/api-gateway/routes/token.router.toml:20-23` 维护着一份自己的网关路由副本，其中 `/v1/tokens/resource` 仍为 `public = true`。若 ai-pm 的部署继续使用这份副本，匿名调用在网关层仍会被透传（但 token-service 侧新门禁仍会 401 兜底——**纵深防御成立，仅网关层未对齐**）。建议随 LMMS 基线更新该副本：删除 `public = true` | 本包 Git 约束禁止修改其他仓库 |
| D-2 | ai-pm | WP-01 集成说明 §3.0 计划的委托链要求用户 JWT 可表达下游受众（aud）——与 JWT-JWKS-ANALYSIS.md §7 建议 3 合并到跨仓库 token 契约裁决 | RECLASSIFICATION 报告 P0-4 → 本报告 ⑤ |
| D-3 | smark-artifact-service | 无行动（已明文不消费；其 `delegation/mod.rs:10` 注释所述"resource token endpoint 未挂载认证"自本修复起不再成立，可在未来分支顺手更新注释，非必须） | — |
| D-4 | smark-sem | 无行动（零使用） | — |

## 4. 结论

| 项 | 结论 |
|---|---|
| Downstream Compatibility | **PASS**——修复零破坏（无真实调用方），纵深防御成立（即便下游网关副本未对齐，token-service 门禁兜底） |
| 职责边界 | **PASS**——下游未把业务授权交给 LMMS；LMMS 未越界承担业务/执行职责 |
| Token 混用 | **PASS**——purpose 分类健康 |
| 契约统一 | **建议项**——aud 语义与 claims 最小集留待跨仓库裁决包 |
