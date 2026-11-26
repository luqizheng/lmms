# WP-LMMS-HARDEN-01-R1 · JWT / JWKS / 服务身份分析

- 审计对象：`lmms` @ `main` = `92691f3313800cc973451e7ca8259753cda666f0`（工作树干净）
- 审计日期：2026-11-26
- 方法：签发端与验证端源码逐行核对 + 既有测试清单核对 + 下游（ai-pm）验证代码交叉核对
- 结论速览：**「用户 JWT 缺 aud」按路径各对一半（登录路径 aud 存在但硬编码；OAuth 路径完全缺失）；「缺 tid」不成立**（access 双路径均由 iam 服务端注入并双重强制，refresh 为例外）

---

## 1. 系统有两条「用户 token」签发路径（裁决前提）

| 路径 | 签发函数 | 入口 | 用途 |
|---|---|---|---|
| A 登录主路径 | `issue_user_jwt`（`services/token-service/src/issuer.rs:183-239`） | iam 登录 → `POST /tokens/user-jwt`（service-jwt 门禁） | 浏览器/SDK 登录用户 JWT |
| B OAuth 路径 | `issue_access_token`（`issuer.rs:104-144`） | `/tokens/access`（authorization_code / client_credentials）与 `/tokens/refresh` 轮换 | OAuth API 访问 |

**审计指控必须按路径拆分裁决，两份历史材料各说中一半。**

## 2. 用户 JWT 实际 claims（代码证实）

### 2.1 路径 A `issue_user_jwt`

| claim | 值 | 证据 |
|---|---|---|
| `sub` | user_id（调用方传入） | `issuer.rs:191` |
| `jti` | `Uuid::new_v4()` | `issuer.rs:192` |
| `iss` | 配置 `TOKEN_ISSUER`（必填非空） | `issuer.rs:193`、`config.rs:96` |
| **`aud`** | **存在，硬编码 `"coder-micro-services"`**（非配置、非请求传入） | `issuer.rs:194`；测试锁定 `tests/issuer_test.rs:200-201` |
| `exp` | now + ttl（默认 3600s） | `issuer.rs:195`、`routes/user_jwt.rs:39` |
| `iat` / `nbf` | **不签发**（jwt-simple `JWTClaims::new()` 不自动盖章，验证端也不校验） | 函数体内无赋值 |
| custom | `scope="access"`、`client_id="credential-login-service"`（钉死）、`user_code`、`purpose="user_login"`（钉死） | `issuer.rs:198-235` |
| `tid` / `ct` / `roles` / `permissions` | **issuer 不写入**，由 iam 组装后经 `claims` body 传入（保留字过滤表不含 tid/ct/roles：`issuer.rs:211-219`） | iam 侧 `iam-service/src/service/auth_svc.rs:209-215,296-302`（`json!({"tid": user.tenant_id, "ct": ..., "roles", "permissions"})`） |

调用方可经 body 注入任意自定义 claims（含 tid/roles），但该端点本身有 service-jwt `aud=token-service` 门禁（`main.rs:232-248`）——注入方是受信平台服务（设计上即 iam），非用户面攻击面。

### 2.2 路径 B `issue_access_token`

| claim | 值 | 证据 |
|---|---|---|
| `sub`/`jti`/`iss`/`exp` | 同上模式 | `issuer.rs:113-116` |
| **`aud`** | **完全不写入** | `issuer.rs:104-144` 无任何 audiences 赋值【代码证实】 |
| `iat` / `nbf` | 不签发 | 同上 |
| custom | `scope="access"`、`client_id`、`purpose`（api_key/auth_code）为权威字段不可覆盖；**tid/ct/roles/permissions 全部来自 extra_claims**（iam `GET /internal/users/{id}/claims` 服务端解析，DTO 改名 `tid`/`ct`：`iam-service/src/dto/user.rs:117-120`） | `issuer.rs:121-139`、`routes/access.rs:146-149` |
| 权限收敛 | client_credentials 的 permissions = 用户权限 ∩ API Key 权限；iam 不可达时 fail-closed 空 permissions | `routes/access.rs:159-186,209-262` |

### 2.3 refresh token（`issue_refresh_token`，`issuer.rs:146-181`）

`sub/jti/iss/exp` + `scope="refresh"` + `family`（UUID，轮换族）+ `purpose` 透传。**不含 tid、client_id、roles、permissions。** jti 落库 marker 防重放（单次使用）。

## 3. 两个原始指控的独立裁决

### 指控 2a「用户 JWT 缺 aud」→ **部分成立（按路径拆分）**

- 登录路径：aud **存在**，但硬编码 `coder-micro-services`——"缺 aud"说法不准确，准确表述是"aud 为固定平台值，无法表达下游受众"。它仍可用作粗粒度受众过滤（audit-service 实际在用：`audit-service/src/routes/mod.rs:69-74`）。
- OAuth 路径：aud **完全缺失**【代码证实】——"缺 aud"对该路径成立。该类 token 可通过 gateway（不校验 aud）与 Principal extractor（默认 aud=None）到达服务，但会被任何 `verify_aud` 门禁（file/iam/audit/token/prue）以 `AudienceMismatch` 拒 401。
- **影响评估**：当前无下游依赖 OAuth token 做受众过滤（DOWNSTREAM-COMPATIBILITY.md §2）；ai-pm 验证 LMMS 用户 JWT 时 `verify_token(token, jwks, None)` 同样不校验 aud。因此不构成本轮 P0/P1，列为 **REDEFINE 设计提案项**（见 §7）。

### 指控 2b「用户 JWT 缺 tid」→ **不成立（对 access token）**

- 登录路径 tid = iam 从 DB 行 `user.tenant_id` 服务端组装；OAuth 路径 tid = iam 服务端按用户查库返回。**用户无法自选租户**。
- 强制点：api-gateway 对所有非 public 路由硬校验 tid 存在性（`api-gateway/src/middleware/mod.rs:274-277`，缺失 401 "missing tenant claim"）；`Principal::from_token` 要求 tid 可解析为 i64（`libs/jwt-ctx/src/principal.rs:129-132`，缺失/非整数报错）。
- 真正的租户隔离在下游 SQL（`QueryBuilder::and_tenant`，`libs/common-db/src/scope.rs:39-47`），与 `docs-site/ai-context.md:200-202` 声称一致。
- **例外（新发现 P1，见 §6.1）**：refresh token 从不含 tid。

## 4. 验证端矩阵（谁验什么）

| 验证方 | 验签 | aud | tid | 其他 |
|---|---|---|---|---|
| api-gateway | JWKS RS256 | **不校验**（`expected_aud=None`，`middleware/mod.rs:265`） | **存在性硬校验**（274-277） | exp 校验（leeway 60s）；iss/nbf 不校验；Redis jti 黑名单 **fail-open**（283-286）；perm-epoch vs iat 撤销 |
| file-service | 自验 | **精确 `file-service`**（`verify_aud_principal_middleware`） | TenantScope 提取 | /file/* 全端点统一门禁，不信任 X 头 |
| iam-service | 自验 | 精确 `iam-service`（内部路由） | 同上 | — |
| audit-service | 自验 | 业务路由精确 `coder-micro-services`（即登录 JWT 的固定 aud 被用作受众过滤）+ 服务路由 `audit-service` | 同上 | — |
| notification-service | 自验 | REST/WS 仅 Principal extractor（**不校验 aud**） | 同上 | 旧 x-user-id 头漏洞已移除 |
| token-service | 自验 | user-jwt/admin/api-keys/keys 精确 `token-service` | 同上 | resource 端点本轮收紧后同列 |
| ai-pm（下游） | 自验（JWKS 指向 token-service） | **不校验**（`verify_token(token, jwks, None)`，`apps/pm-api/src/auth.rs:289`） | **强制存在**（缺失 401） | 拒 `roles=["service"]` 与 `ct=true`；`purpose` 权威分类（service/resource 拒绝、未知枚举 fail-closed） |

X-User-* 注入头：全仓源码零命中（`from_headers` 纯解码函数仅测试使用），与 `ai-context.md:99` "禁止读 X-User-* headers" 一致。

## 5. 服务身份与 JWKS

- **服务身份链**：bootstrap token（明文落库 `token_bootstrap_tokens`，owner 唯一）→ `POST /tokens/service-jwt` → service-jwt（`iss="token-service"`、`sub`=owner、`aud`=请求指定目标服务、`tid`=请求指定、`roles=["service"]`、`purpose="service"`、可选 `ct:true`，1h TTL；`routes/service_jwt.rs:71-98`）。
- **用户/服务 token 四要素区分**：`purpose`（user_login/api_key/auth_code/service/resource）+ `sub` 格式 + `aud`（目标服务 vs 固定平台值）+ `roles=["service"]`。出站 `mint_service_bearer_*` 助手同构。
- **密钥生命周期**：`active`（唯一签名者）→ rotate 降 `retiring`（verify-only，仍在 JWKS）→ retire（移出 JWKS）→ delete（30 天保留期门禁）。kid = 公钥 DER SHA-256 前 16 字节 hex；rotate 原子落盘 + 进程内热更新；损坏即拒绝启动。
- **JWKS 端点**：`GET /.well-known/jwks.json`（别名 `/tokens/.well-known/jwks.json`），只发布 active+retiring；算法白名单**仅 RS256**（alg=none/HS256 混淆不可行）；kid 严格匹配，未知 kid → 401（有测试）；无 kid 头的遗留 token 回落 `kid="default"`（兼容窗口，仅历史密钥）；消费方缓存刷新周期 3600s（轮换后最长 1h 滞后窗口）；token-service 自身 rotate 后免 HTTP 热更新。

## 6. 新发现问题分级

### P1-1 refresh token 无 tid + 解码缺省租户 1（功能缺陷，安全影响有限）

`routes/refresh.rs:70-74` 解码 refresh token 时 `tid` 缺省取 **1**，随后用 `?tenant_id=1` 回查 iam claims（143-146）；iam 按租户过滤用户行（`user_svc.rs:974-979` `and_tenant`）。由于签发端从不写 tid（git 历史无记录），**tenant≠1 的用户走 OAuth refresh 会 404→503**。iam 自管的登录 refresh（不透明 token 存 DB，`auth_svc.rs:223-237`）不受影响。

**处置**：不在本包改代码——修复需要"refresh token 写入 tid"或"回查去租户过滤"的设计决策（影响 token 契约与存量 token 兼容），按任务书 §9 应先提交设计提案。已记入 AUDIT-REPORT 建议项。

### P2（记录，不实施）

| # | 发现 | 证据 | 说明 |
|---|---|---|---|
| P2-1 | gateway 不校验 aud/iss/nbf | `middleware/mod.rs:265`、JwtClaims 无 aud 字段（99-111） | 已知折衷；下游 aud 门禁兜底 |
| P2-2 | gateway Redis 黑名单 fail-open | `middleware/mod.rs:283-286` | Redis 不可用时撤销失效；有 TTL 短期缓解 |
| P2-3 | `?token=` query 传 token | `middleware/mod.rs:240-252` | token 可能进日志/Referer |
| P2-4 | OAuth access token 无 aud | `issuer.rs:104-144` | 见 §3；与 P1-1 同属 token 契约设计提案 |
| P2-5 | iat/nbf 不签发不校验 | 签发/验证两端 | perm-epoch 撤销机制依赖 iat 时部分降级 |
| P2-6 | `/tokens/user-jwt` claims 注入保留字不含 tid/ct/roles | `issuer.rs:211-219` | 注入方受 service-jwt 门禁约束（受信服务面），接受为设计内 |

## 7. 测试覆盖与建议

**现有测试（已核对源码）**：签发 claims（issuer_test 7 项，含 aud 硬编码锁定、保留字过滤、purpose 权威性）；service_jwt_test（合法/错 bootstrap/无 auth/空 aud/禁用+过期）；common-jwt verify 负面四件套（过期/错 aud/篡改/未知 kid）；jwt-ctx（tid 解析/缺失/非整数、aud/tamper）；authz（wrong_aud 401 等 4 项）；file-service（用户 JWT 直连 401）。

**缺口（未找到）**：gateway 层 tid 缺失分支无 gateway 级测试；OAuth access token 的 aud 行为无任何测试；无 refresh-token-无-tid 的多租户负面测试。

**建议（设计提案，需人工批准后另立工作包）**：
1. refresh token 签发时写入 tid（或回查去租户过滤），消除 P1-1；
2. `issue_access_token` 增加 aud 语义（为 OAuth token 定义受众模型）；
3. `issue_user_jwt` 的 aud 是否需要可配置（AI-PM 委托链期望 aud=artifact-service 的用户 JWT 是其 P0-02 阻断项）——属跨仓库契约决策，不在 LMMS 单方面改动。
