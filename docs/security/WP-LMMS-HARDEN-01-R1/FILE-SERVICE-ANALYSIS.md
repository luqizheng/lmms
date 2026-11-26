# WP-LMMS-HARDEN-01-R1 · File Service 资源访问分析

- 审计对象：`lmms` @ `main` = `92691f3313800cc973451e7ca8259753cda666f0`
- 审计日期：2026-11-26
- 结论速览：**file-service 已收口为纯内部服务，认证与租户隔离有代码+测试保障；LMMS resource token 与 file-service 完全没有对接（签发端是孤儿功能）；审计事件承诺未兑现是最大文档-代码背离**

---

## 1. 端点与认证（代码证实）

`/file/*` 端点（`services/file-service/src/routes/file.rs:14-18`、`presign.rs:8`、`migration_state.rs:86`）：

| 端点 | 方法 | 认证 |
|---|---|---|
| `/file/upload` | POST | `verify_aud_principal_middleware("file-service")` |
| `/file/list` | GET | 同上 |
| `/file/{id}`（下载）/ DELETE / `/file/{id}/info` | GET/DELETE/GET | 同上 |
| `/file/presign` | POST | 同上 |
| `/file/internal/migrations/state` | GET | 同上 |
| `/health`、`/metrics` | GET | 无（探针） |

- 门禁统一挂载于 `routes/mod.rs:33-45`；实现为 `libs/authz/src/verify_aud.rs:47` → `Principal::from_token(&token, &cache, Some("file-service"))`，任何失败 401。
- 验签：`libs/common-jwt/src/verify.rs:87-94`——RS256 + kid 严格 + **aud 精确匹配**。**用户 JWT（aud=coder-micro-services）直连 file-service 一律 401**，有负面测试（`tests/tenant_isolation_test.rs:265` `user_jwt_rejected_on_file_routes`）。
- api-gateway 用户面路由已全部撤下（`examples/docker-deploy/config/api-gateway/routes/file.router.toml:1-9` 注释："file-service 收口为纯内部服务"；仅剩 `/v1/file/health` public）。**gateway 自身验 JWT 不校验 aud**（`middleware/mod.rs:265`）——但 file-service 的路由层门禁是权威防线。
- **绕过 gateway 直连**：仍有认证（aud 门禁在路由层）。file-service 全部源码无任何 `X-User-*`/`X-Tenant-*` 读取（grep 零命中），身份完全取自验签后的 token claims（`libs/jwt-ctx/src/scope.rs:25-30`）。
- 注意边界：任何持 `aud=file-service` service-jwt 的调用方都被接受，file-service 不区分"哪个服务"（`sub` 仅记录，无调用方白名单）——符合"验调用方类别，不验业务身份"的基础设施定位。

## 2. 租户隔离（代码证实）

- 全部读写路径强制 `AND tenant_id = ?`：列表 `services/file-service/src/services/file.rs:216-217`（`qb.and_tenant(scope)`）、`get_file_info`（232-235）、`delete_file_info`（303-306）；下载/presign 均先经 `get_file_info`。
- `CrossTenant`（`ct=true`）为 no-op（`libs/common-db/src/scope.rs:44`）——平台级通行证，**设计内**（`tenant_isolation_test.rs:6-7` 注释印证），但意味着任何 `ct=true` service-jwt 可跨租户读写全部文件，签发面收口在 bootstrap token 管理面。
- 文件 ID 为 UUID v4（`file.rs:78`），不可枚举；越权 ID 在单租户 token 下返回 404（有测试：t1 token 读 t2 文件 404，`tenant_isolation_test.rs:213-217,254-258`）。
- **小缺陷（P2-1）**：`ct=true` 上传时 `tenant_id: scope.tenant_id().unwrap_or(1)`（`file.rs:90`）——跨租户 token 上传静默落入租户 1，而非拒绝或显式指定。建议改为显式必填或拒绝。

## 3. 授权模型（代码证实 + 文档契约）

- `tenant_id`：来自 service-jwt `tid` claim。请求体/查询参数中的 tenant_id **不被采信**（`UploadFileMeta.tenant_id` 字段存在但代码不读取；`QueryFiles.tenant_id` 解析后 service 层不使用）——无"参数覆盖租户"路径。
- `user_id`：**完全采信调用方 metadata**（`file.rs:91`）。这是文档明确的设计契约：`contracts/http-conventions.md:62-71`"调用方对声明的 user_id 的正确性负责…file-service 只验调用方"；`docs-site/guides/file-storage-usage.md:88`"调用服务必须在鉴权后把真实业务归属写入 metadata"。
- **与任务书 §2 原则的关系**：符合"业务授权在业务系统、LMMS 只验基础设施调用方"。业务系统（如 ai-pm）持 per-tenant service-jwt（`tid=<租户>`）代表已授权用户写文件——授权决策留在业务侧，LMMS 不判断业务归属。✓
- 下游实况（交叉证据）：ai-pm `apps/pm-api/src/files.rs:57-117` 每请求经 `ServiceJwtCache.get_for_tenant("file-service", tid, false)` 换取 per-tenant service-jwt 后代理上传——即"业务系统先做业务授权，再以受限服务身份调用 LMMS"的正确姿势。

## 4. presign（代码证实）

- 仅 `aud=file-service` service-jwt 可创建（同门禁；用户 JWT 请求 presign 401 有测试）。
- 操作白名单 `{get, put}` 严格反序列化（`models/presign_url.rs:8-13`；`"download"`/`"delete"` 拒绝有测试）。
- TTL 限制 `1..=604800`（`file.rs:15,153-157`）；临时文件 TTL 夹到文件过期时间（160-166）。
- LocalStorage 无原生 presign → fail-closed 501（`storage/object_store.rs:26-33` 默认 None → `file.rs:181-186` → 501，符合约定）。
- S3 对象 key = 裸 UUID，无租户前缀（`s3.rs:175-180`）——URL 是 bearer capability，签发后不受租户体系约束，仅靠签名+TTL+对象删除失效（`file.rs:133-135` 注释明示）。presign 签发留痕于 `file_presign_urls` 表但 `created_by: None`（`file.rs:195`，无调用方署名，P2-2）。

## 5. 直接存储访问（代码证实）

- LocalStorage 根 `STORAGE_PATH`（默认 `./data/files`）；路径拼接后 `ensure_within_base` canonicalize 防穿越（`storage/local.rs:26-40`，`FILE_STORAGE_4101`），get/put/delete 全部调用。实际可穿越面极小：进入存储层的 path 只有服务端自产 UUID，客户端无直接路径输入。
- S3/RustFS 桶凭据为整桶粒度（`s3.rs:64-71`），非按租户隔离——持有桶凭据即绕过 file-service 全部控制；属部署面信任边界，记录备查。
- 文件字节在 Local 后端只能经 API 到达（容器卷除外）。

## 6. resource token 与 file-service：【未找到】——关键结论

- token-service 有签发侧（`routes/resource.rs` → `issuer.issue_resource_token`，claims `ResourceTokenClaims`：`libs/common/src/token.rs:27-45`）。
- **file-service 侧验证代码不存在**：repo-wide grep `ResourceTokenClaims` 仅命中定义文件；file-service 不依赖 `digua-common`，无任何 resource token 解析逻辑。且 resource token 本身**无 aud claim**，会被 file-service 的 aud 门禁以 `AudienceMismatch` 401 拒绝。
- **结论：LMMS resource token 当前无法用于访问任何资源（全仓无验证方）**。file-service 的唯一令牌入口是 `aud=file-service` 的 service-jwt 与 S3 presign URL。这直接否定了"下游可用 resource token 访问文件"的假设，也意味着 WP-01 审计所称"resource token 未鉴权"的实际暴露面 = 签名神谕（见 RESOURCE-TOKEN-ANALYSIS.md），而非资源越权。

## 7. 审计与事件（文档-代码背离，P2-3）

- `contracts/events.yaml:106-126` 声明 file-service 产出 `file.uploaded`/`file.deleted`；`http-conventions.md:71`"记账照单全收并写审计"。
- **代码未实现**：file-service 无任何事件发布代码（grep `outbox|emit|publish` 零命中，不依赖 common-event）；audit-service 只订阅 `iam.#` 与 `pmd.#`（`consumer.rs:6-7`），无 `file.#` 绑定。
- 现存留痕仅：`file_presign_urls` 表 + tracing/OpenTelemetry 请求链路日志（非审计事件）。

## 8. 测试清单（现有）与缺口

**现有（已核对）**：
- `tenant_isolation_test.rs`：跨租户 404（双向）、`ct=true` 全通、用户 JWT 401（info+presign 两处）。
- `libs/authz/tests/verify_aud_test.rs`：wrong_aud/missing_auth/bad_signature 401、valid 放行。
- `models_test.rs`：presign 操作白名单严格反序列化。
- `local_storage_test.rs`：删除幂等。
- `pg_roundtrip_test.rs`：类型/分页（非安全向）。

**缺口**：无伪造 metadata.user_id 越权记账负面测试（设计上无法防，但可固化行为）；无 LocalStorage 路径穿越测试（`ensure_within_base` 无直接测试）；无 presign URL 越权使用测试；`file-storage-usage.md:248` 的跨租户防护清单项为未勾选 `[ ]` 状态。

## 9. 结论

| 项 | 结论 |
|---|---|
| 认证门禁 | **PASS**（aud 精确匹配 + RS256 + kid 严格 + 负面测试） |
| 租户隔离 | **PASS**（SQL 强制 + UUID 不可枚举 + 测试）；`ct=true` 为设计内通行证 |
| 业务授权边界 | **PASS**（user_id 采信调用方 = 文档契约；业务授权留在业务系统） |
| resource token 对接 | **N/A——不存在**（孤儿功能，file-service 不接受） |
| 审计链路 | **FAIL（文档承诺未兑现）**——P2-3，建议另立工作包兑现或修订文档 |
| 建议修复 | P2-1 `ct=true` 上传落租户 1 改显式/拒绝；P2-2 presign `created_by` 署名；补路径穿越与伪造 metadata 测试（均可并入后续工作包，不属本包 P0/P1） |
