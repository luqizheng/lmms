# WP-LMMS-HARDEN-01-R1 · LMMS Resource Token、身份认证与基础设施授权边界审计报告

| 项 | 值 |
|---|---|
| 任务 | WP-LMMS-HARDEN-01-R1 · LMMS 独立安全审计与必要修复 |
| 审计日期 | 2026-11-26 |
| 审计基线 | `lmms` @ `main` = `92691f3313800cc973451e7ca8259753cda666f0`（工作树干净） |
| 修复分支 | `fix/wp-lmms-harden-01-r1`（独立 worktree `C:\projects\lmms-harden-01`，基于上述基线） |
| 下游核查对象 | `ai-pm` @ `6f101ce9`（master 基线 `6f317176`）、`smark-artifact-service` @ `8fd5173`（r1 树 `b40bc9d`）、`smark-sem` @ `344a9da`（只读克隆） |
| 前置输入 | `smark-artifact-service` @ `docs/wp-lmms-delegation-reclassification-01`（commit `eba3078e`，blob `da183c5`）`docs/security/WP-LMMS-DELEGATION-RECLASSIFICATION-01/AUDIT-REPORT.md` 中两项"待复核"指控——已复核可达 |
| 子报告 | RESOURCE-TOKEN-ANALYSIS.md · JWT-JWKS-ANALYSIS.md · FILE-SERVICE-ANALYSIS.md · DOWNSTREAM-COMPATIBILITY.md |

---

## 1. 检查范围与证据基础

**源码**：token-service（routes/issuer/service/store/keystore 全量）、api-gateway 中间件、file-service 路由/服务/存储、libs/{authz, jwt-ctx, common-jwt, common-db}、Java/Rust SDK；下游三仓库对 LMMS 的调用点与验证点。
**文档**：README、AGENTS、ai-context、token-service-overview、file-storage-usage、contracts/{events.yaml, http-conventions}、specs/{2025-07-11, 2026-11-04}、FEATURES。
**测试**：token-service 17 个集成测试目标 + 4 个支撑库测试文件逐项核对。
**CI**：GitHub Actions 3 个工作流（push main/develop + PR→main 触发）；GitLab CI **全局暂停**（`.gitlab-ci.yml` workflow 门禁 `LMMS_CI_ENABLED`）。

## 2. `/tokens/resource` 原始设计目的

签发**单一资源、单一权限、短 TTL（默认 300s）** 的受限 JWT（`purpose=resource`），供资源服务做细粒度临时授权；设计规范明确该端点「JWT 验证 = 是」（`specs/2025-07-11:415`）。完整分析见 RESOURCE-TOKEN-ANALYSIS.md。

## 3. 原始两个 P0 指控的独立结论

### 指控 1「`/tokens/resource` 可能缺少有效认证或授权」→ **成立【代码证实】**

- token-service 侧：路由无中间件（`main.rs:188-191`），handler 仅 `State`+`Json`（`routes/resource.rs:21-24`）——是签发类端点中唯一零认证者。
- 网关侧：`/v1/tokens/resource` 配置 `public = true`（两份路由副本一致）。
- 后果：**公开签名神谕**——输入零验证（resource_type/resource_id/permission 原样进 claims、TTL 无上限），任意匿名调用方可无限量获取平台可信 RS256 密钥签名的任意声明 JWT。
- 缓解因素：**全仓零消费方**（`ResourceTokenClaims` 零使用者；file-service 因 token 无 aud 而 401；网关保护路由因无 tid 而 401；撤销黑名单无执行点读取）——直接利用面有限，但神谕本身是未来任何 token 验证逻辑的隐患。

### 指控 2「用户 JWT 可能缺少 aud、tid 等必要 claims」→ **部分成立（必须按签发路径拆分）**

- **aud**：登录路径（`issue_user_jwt`）**存在但硬编码** `"coder-micro-services"`（`issuer.rs:194`，测试锁定）——"缺 aud"表述不准确，准确说法是"固定平台值、无法表达下游受众"；OAuth 路径（`issue_access_token`，含 refresh 轮换）**完全无 aud**（`issuer.rs:104-144`）——该半句成立。
- **tid**：**指控不成立**（对 access token）——登录与 OAuth 双路径的 tid 均由 iam 服务端从 DB 注入（用户无法自选），api-gateway（`middleware/mod.rs:274-277`）与 `Principal`（`jwt-ctx/principal.rs:129-132`）双重强制存在性，缺失即 401。
- **例外（新发现）**：refresh token 从不含 tid，且解码缺省租户 1（见 §5 P1-1）。

## 4. 业务授权与基础设施授权边界（§4.3 A/B/C）

- **A 业务系统授权决策**：下游实况健康——ai-pm 自持业务授权（租户/角色取库、权限码直比、拒 service/ct token），AS 自持委托授权；未发现把业务授权交给 LMMS 的代码。
- **B LMMS 验证申请方与基础设施访问范围**：file-service 正确（aud=file-service 门禁 + SQL 租户过滤 + 用户 JWT 401 有测试）；**`/tokens/resource` 修复前为唯一失守点（B 层全缺），已修**。
- **C 资源服务执行限制**：resource token 无验证实现（孤儿令牌）——当前不存在被绕过的资源面；未来对接前必须先建验证端。

## 5. 新发现问题清单（原始两项之外）

| 级别 | 发现 | 证据 | 本包处置 |
|---|---|---|---|
| **P1-1** | refresh token 无 tid + 解码缺省租户 1：tenant≠1 用户 OAuth 刷新回查 iam 按 tenant 1 过滤 → 404→503（功能缺陷；iam 自管登录刷新不受影响） | `routes/refresh.rs:70-74,143-165`、`issuer.rs:146-181` | **不修**——需 token 契约设计提案（写 tid 或回查去租户过滤），按任务书 §9 留待批准 |
| P2-1 | `ct=true` service-jwt 上传文件静默落入租户 1（`unwrap_or(1)`） | `file-service/services/file.rs:90` | 建议改显式必填/拒绝，另立工作包 |
| P2-2 | file-service 审计事件承诺未兑现：`events.yaml` 声明 `file.uploaded/deleted`、文档称"写审计"，但无发布代码且 audit-service 不订阅 `file.#` | `contracts/events.yaml:106-126` vs 全仓 grep | 建议兑现或修订文档，另立工作包 |
| P2-3 | gateway 不校验 aud/iss/nbf；Redis 黑名单 fail-open；`?token=` query 传 token | `middleware/mod.rs:240-314` | 已知折衷，记录备查 |
| P2-4 | presign `created_by: None` 无调用方署名；路径穿越防护无测试；伪造 metadata.user_id 无固化测试 | `file.rs:195`、`local.rs:26-40` | 测试缺口清单，另立工作包 |
| P2-5 | resource token TTL 无上限、permission 无值域、撤销链路断裂 | `resource.rs:25`、`revoke_svc.rs:23-30` | 随 Resource Token REDEFINE 提案 |
| P2-6 | gateway 层 tid 缺失分支无 gateway 级测试；OAuth token aud 行为零测试 | gateway tests grep tid 零命中 | 测试缺口清单 |

## 6. 修复决策与实施（P0-1 → FIX）

- **决策**：指控 1 为确认 P0 → 实施最小修复；指控 2 不构成代码修复项（aud 属契约设计、tid 指控不成立、refresh 缺陷属 P1 功能缺陷需提案）。
- **修复范围**（不做无关重构、不破坏合法调用方、零数据库迁移）：见 RESOURCE-TOKEN-ANALYSIS.md §6 的 5 文件 + 1 新测试清单。核心 = `/tokens/resource` 挂 `verify_aud_principal_middleware("token-service")`（完全镜像 `/tokens/user-jwt` 与 `/admin/*` 既有模式）+ 网关 `public` 翻转 + 4 处文档同步。
- **明确不做**：token claims 契约变更（aud/tid/调用方绑定）、TTL 封顶、permission 值域、撤销链路接通——全部属 REDEFINE 提案，待人工批准。

## 7. 测试命令与结果（已执行 vs 未执行严格区分）

**已执行**：
```
cargo fmt -p token_service                                        # ✓
cargo clippy -p token_service --all-targets                       # ✓（无新增警告）
cargo test -p token_service --lib                                 # ✓ 79 passed（修复后）
cargo test -p token_service --test resource_token_auth_test       # ✓ 3 passed（新增回归）
TEST_MYSQL_HOST=127.0.0.1 TEST_MYSQL_PORT=13306 \
CARGO_TARGET_DIR=/c/projects/lmms/target cargo test -p token_service
# ✓ REAL_EXIT=0：lib 79 + 集成 91 = 170 passed / 0 failed
```
集成分项：api_key_auth 5 · api_key 3 · auth_code 5 · bootstrap_token_admin 12 · bootstrap_token_web 14 · client_credentials 7 · integration 1 · issuer 8 · migration_marker 1 · oauth_client 13 · pg_roundtrip **0（共享 MySQL 模式按设计跳过）** · refresh_marker 3 · **resource_token_auth 3（新增）** · revoke 4 · service_jwt 10 · swagger 2。

**未执行（如实声明）**：file-service / api-gateway / authz 等其他 crate 的测试套件（本包代码未触及，基线 6 crate lib 已于审计首日全绿）；OpenAPI 生成门禁（中间件变更不影响 utoipa spec，但未跑生成比对）；**CI 未运行**——修复分支 push 不触发 GH Actions（仅 main/develop + PR→main），GitLab CI 全局暂停；独立评审时请开 PR 触发。

**环境备注（复现须知）**：本机 GitHub HTTPS 不可达（仅 SSH）→ `utoipa-swagger-ui` 构建需复用缓存（本次以 `CARGO_TARGET_DIR` 指向主 worktree 缓存解决）；testcontainers 在此 Docker Desktop 上管道直连失败（OS error 53，默认管道与 `desktop-linux` context 管道不一致）→ 集成测试需共享 MySQL（`TEST_MYSQL_HOST/PORT`，本次为临时容器 `lmms-harden01-mysql` @ 127.0.0.1:13306，root 空密码）或修复 `DOCKER_HOST`。

## 8. 未解决风险（移交后续）

1. Resource Token 契约缺失（无 aud/tid/调用方绑定/撤销）——**在其验证方存在之前不得投入使用**；需 REDEFINE 提案或 DEPRECATE 计划。
2. P1-1 refresh 多租户刷新缺陷——待设计提案。
3. 下游行动项 D-1：ai-pm 部署配置副本中 `/v1/tokens/resource` 仍 `public=true`（token-service 门禁兜底，纵深防御成立）——需下游工作包对齐。
4. file-service 审计链路缺失（P2-2）。
5. CI 对本分支不可见——独立评审必须经 PR 触发流水线回填。

## 9. 发布建议

- **合并路径**：经 PR → main 触发 CI 全量验证后合并；不建议直接合（本机未跑 CI）。
- **发布顺序**：token-service 修复与网关路由配置**同批发布**（避免只发其一造成网关放行/服务拒绝的告警噪音——但即便乱序，安全语义仍由 token-service 门禁保证）。
- **下游通知**：按 DOWNSTREAM-COMPATIBILITY.md §3 清单通知 ai-pm（D-1 配置对齐）；Java SDK 文档补一句"issueResource 须持 aud=token-service service-jwt"（非阻塞）。
- **前提保持**：AI-PM 委托链等下游使用 LMMS 用户 JWT/JWKS 的方案不受本修复影响（零改动面）。

## 10. 最终验收结论（任务书 §12）

| 审计项目 | 结论 |
|---|---|
| Resource Token 设计合理性 | **FIX**（已修认证；契约语义 REDEFINE 提案待批） |
| Resource Token 安全性 | **PASS**（修复后：神谕入口关闭 + 3 例回归锁定；令牌不得在验证方存在前投入使用） |
| 业务授权边界 | **PASS**（A/B/C 三层职责清晰；下游无越权交付） |
| JWT Claims | **CONDITIONAL**——tid/iss/exp 链健康；aud 按 JWT-JWKS-ANALYSIS §3 拆分结论（OAuth 路径缺 aud、登录路径固定值）+ P1-1 refresh 缺陷待提案 |
| 服务身份 | **PASS**（bootstrap→service-jwt 链完整，purpose/roles/sub/aud 四要素区分，轮换机制完备） |
| JWKS | **PASS**（RS256 白名单、kid 严格、active/retiring/retired 状态机、30 天保留、负面测试齐全） |
| File Service | **PASS**（纯内部收口 + aud 门禁 + SQL 租户隔离 + 测试；审计事件缺失记 P2-2） |
| Tenant Isolation | **PASS**（SQL 强制 + UUID 不可枚举 + 双向 404 测试；`ct=true` 为设计内通行证） |
| Downstream Compatibility | **PASS**（零下游调用方 → 零破坏；ai-pm 配置副本对齐为下游行动项 D-1） |
| **Overall Readiness** | **CONDITIONAL**——无未解决 P0；存在 1 项 P1（refresh tid，功能缺陷）与若干 P2 待提案/待工作包，且本分支 CI 未经流水线验证，故不给无条件 READY |

**声明**：本报告不宣布 LMMS/AI-PM/Smark-SEM/Artifact Service 跨仓委托链路已通过验收——委托链的 AI-PM 侧实现仍为零（见 RECLASSIFICATION 报告），链路级验收不在本包范围。最终交付后等待独立评审。
