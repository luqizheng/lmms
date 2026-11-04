# token-service 增加 `purpose` claim 设计稿 V1.0

- 日期：2026-11-04
- 状态：待评审（先核验后设计，未写码）
- 服务范围：token-service（生产签发口唯一，见 §2.4）
- 下游驱动：ai-pm S02-I6-A 裁决 A*（正式确认仅限人类交互会话，服务端须能区分人类浏览器会话与 Agent 持有人类凭据，无法区分时 fail-closed）

---

## 1. 背景与动机

下游 ai-pm 的 `pm-api` 目前靠 JWT 里的 `aud` / `client_id` **推断**令牌类别
（`auth.rs` `auth_mode()`：有 `aud`→interactive，无 `aud` 有 `client_id`→api_key）。
该推断存在致命缺口：Agent 持真人**登录 JWT**（含 `aud`）与浏览器会话在服务端
**无任何可检差异**，且现行 MCP 部署用的就是 `/v1/iam/auth/login` 登录令牌。

解法：token-service 在签发时写入**服务端权威的用途声明** `purpose`（由签发通道决定，
客户端不可自报），下游由「推断」升级为「读取声明」。语义边界（签发通道 ≠
当前持有人身份）与其对 A* 生效前提的影响见 §4.9。

## 2. 现状代码事实（签发链路清单）

### 2.1 签发函数（`services/token-service/src/issuer.rs`）

| 函数 | 行号 | 现有 custom claims | aud | 用途 |
|---|---|---|---|---|
| `issue_access_token` | issuer.rs:36-68 | `scope=access`、`client_id=<入参>`、合并 extra_claims（**合并可覆盖前置键**） | 无 | OAuth 两流 + refresh 的新 access token |
| `issue_refresh_token` | issuer.rs:70-95 | `scope=refresh`、`family=<uuid>` | 无 | authorization_code 流的 refresh token |
| `issue_user_jwt` | issuer.rs:97-137 | `scope=access`、`client_id=credential-login-service`、`user_code`、合并 extra_claims（过滤保留键 `iss/aud/exp/iat/nbf/sub/jti`） | `coder-micro-services` | **登录路径**（iam 登录 + iam refresh） |
| `issue_resource_token` | issuer.rs:139-174 | `scope=resource`、`resource_type/id`、`permission` | 无 | 资源受限令牌 |

### 2.2 签发路由与调用链

| 通道 | 入口 | 鉴权 | 落到哪个签发函数 | business claims 来源 |
|---|---|---|---|---|
| **登录** | iam `/v1/iam/auth/login`（auth_svc.rs:125-137）→ token_forward.rs:53-85 → token-service `POST /tokens/user-jwt`（user_jwt.rs:35-48） | service-jwt `aud=token-service`（main.rs:186-199） | `issue_user_jwt` | iam 硬编码 `{tid, ct, username, roles, permissions}`（auth_svc.rs:125-131） |
| **iam refresh** | iam `refresh`（auth_svc.rs:246-297，不透明 nanoid 令牌存 `iam_refresh_tokens`） | 同上 | `issue_user_jwt` | 同上（:285-291 重建 claims） |
| **client_credentials** | `POST /tokens/access` grant_type=client_credentials（access.rs:117-191） | client_secret（argon2，body 内） | `issue_access_token` | iam `GET /internal/users/{id}/claims`（**fail-closed**：不可达→permissions 清空，access.rs:164-171）；`sub`=API Key 绑定 user_id；permissions=用户∩钥匙（access.rs:194-199）；**不签 refresh**（RFC 6749 §4.4.3） |
| **authorization_code** | `POST /tokens/access` grant_type=authorization_code（access.rs:201-262） | client_secret | `issue_access_token` + `issue_refresh_token` | 同一 claims 端点，但**fail-open**（不可达→兜底 `roles:["admin"]`，access.rs:235-239，见 §8） |
| **OAuth refresh** | `POST /tokens/refresh`（refresh.rs:83-155） | client_secret + refresh JWT 防重放（refresh_marker） | `issue_access_token` | claims 端点，不可达→503（fail-closed，refresh.rs:126-133） |
| **service-jwt** | `POST /tokens/service-jwt`（service_jwt.rs:39-83） | 预共享 bootstrap token | `digua_common_jwt::sign_token`（common-jwt/verify.rs:32-46，独立签名路径） | 硬编码 `{iss, sub=SERVICE_NAME, aud=<目标服务>, tid, roles:["service"]}` |
| **（例外）本地铸造** | `ServiceJwtCache::sign_local`（common-jwt/service_jwt.rs:145-160） | 持私钥即可 | `sign_token` | claims 与端点路径一致；仅测试调用，不盖章（§2.4） |
| **resource** | `POST /tokens/resource`（resource.rs） | — | `issue_resource_token` | — |

### 2.3 关键消费端点（iam 侧）

- `GET /internal/users/{id}/claims`（iam routes/users.rs:24）→ `UserClaimsResponse`
  字段：`user_id/username/nickname/email/phone/roles/permissions/tid/ct`
  （dto/user.rs:98-110）。**无 `purpose`、无 `scope`/`client_id` 键** → 现状合并无冲突；
  但设计仍按「先合并、后盖章」硬化（§4.3）。

### 2.4 签发口封闭性（本稿地基）

生产代码中 JWT 签名**只有两处**，均在 token-service：
1. `TokenIssuer`（issuer.rs，§2.1 四函数）；iam-service 里的 `issue_user_jwt` 调用
   是 `TokenForwardClient` 的 HTTP 转发（token_forward.rs:53），不本地签名；
2. `digua_common_jwt::sign_token`（仅 service_jwt.rs:71 使用）。

仓内其余 `sign_token/RS256KeyPair/EncodingKey` 命中全部为各服务**测试文件**
（构造令牌验签用），另有一处（builder check 发现，见下）。

**已知例外**：`ServiceJwtCache::sign_local` / `prime_cache`
（libs/common-jwt/src/service_jwt.rs:129-160，`#[doc(hidden)]` 自述 test-only）
用 token-service 私钥本地铸造 service-jwt，绕过 `/tokens/service-jwt` 端点；
现行调用方全部为测试（common-jwt/tests、notification-service/tests/iam_client_test.rs:35）。
定性：**不构成越权旁路**——持有私钥本即拥有任意签名权（含伪造任意 purpose），
`sign_local` 不新增特权。处置：本稿**不盖章**（保持影响面单 crate），在 §4.4
矩阵外以本段存档；搬入 test-util crate 或 `#[cfg(test)]` 化列为 §8 清理候选。

## 3. 旧结论复核（此前记录 → 现状）

| 旧结论 | 复核结果 | 证据 |
|---|---|---|
| client_credentials 为既定未实现缺口 | **过时，已实现**（方案 A：sub=绑定 user_id、权限交集、claims 拉取 fail-closed、无 refresh） | access.rs:112-199；api_key.rs:32-34 默认 grant_types 即含它 |
| API Key 无 user_id、无过期 | **过时**。`user_id: Option<String>`（client_credentials 必填绑定，access.rs:131-134）、`expires_at: Option<String>`（缺省不过期） | api_key.rs:24-29、admin.rs:83-88 |
| `/api-keys` 端点无鉴权 | **已修复**。service-jwt `aud=token-service` 中间件（注释自述「原先裸挂」后补） | api_key.rs:156-172；admin.rs:315-342 同批 |
| SDK 用表单编码 | **仍成立**。`client_credentials` 走 `application/x-www-form-urlencoded`；服务端按 Content-Type 双编码兼容 | sdk/rust/digua-http-client/src/fetcher.rs:27-33；access.rs:59-72 |

附注：FEATURES.md 端点表路径（`/api/tokens/...`）与实际路由（`/tokens/...`）不一致，
文档陈旧，不阻塞本稿。

## 4. 设计

### 4.1 claim 命名：`purpose`（String）

- 选型理由：语义自解释（"这张令牌为何被签发"）；RFC 8693（Token Exchange）已有
  `purpose` 先例；与下游 S02 文档现行称呼一致，下游契约零翻译。
- 否决项：`token_type`（与 OAuth 响应字段 `token_type: Bearer` 撞名）；`typ`
  （JWT header 已有）；`grant`（值域含非 grant 通道的 service/resource）。
- 位置：JWT payload 顶层 custom 键，与 `tid/ct/client_id` 同级（短名风格协调）。

### 4.2 值域（封闭枚举，5 值）

| 值 | 签发通道 | 对应 §2.2 行 |
|---|---|---|
| `user_login` | `/tokens/user-jwt`（登录 + iam refresh） | 登录 / iam refresh |
| `api_key` | client_credentials 兑换 | client_credentials |
| `auth_code` | authorization_code 兑换 | authorization_code |
| `service` | service-jwt（aud=<目标服务>） | service-jwt |
| `resource` | resource token | resource |

要点：
- **iam refresh 不单设值**：与登录同通道，继承 `user_login`（改的只是 access token
  的续期，不代表新一次人类在场）。
- `auth_code` 与 `api_key` 分开：两者都「机器代用户」，但凭据类不同（OAuth 客户端
  vs API Key），留给下游与未来策略区分；下游 A* 判据下两者同属「非人类」。
- 未列值一律非法；下游遇到未知值按 fail-closed 处理（枚举封闭性的意义）。

### 4.3 防伪不变量（硬性安全要求 1 的落实）

1. **通道决定值**：`purpose` 只在 issuer 签名路径内写入，值为编译期常量/入参枚举，
   不存在任何「从请求体读 purpose」的代码路径。
2. **后写覆盖（last-write-wins）**：`issue_access_token` / `issue_user_jwt` 改为
   「先合并 extra_claims、后写权威键（`scope`/`client_id`/`purpose`）」。现状
   issuer.rs:49-63 的合并可覆盖前置键，借本次一并硬化——claims 端点今天虽不返回
   冲突键（§2.3），不变量不依赖上游行为。
3. **保留键过滤**：`issue_user_jwt` 的 reserved 列表（issuer.rs:125）追加
   `"purpose"`——即使内部调用方（iam）在 claims 里带了 purpose 也被丢弃。
4. `user_jwt.rs` 的 `UserJwtRequest` **不新增** purpose 字段；`api_key.rs` /
   `access.rs` 请求体同样不收。

### 4.4 签发点改动矩阵

| 签发函数 | 改动 |
|---|---|
| `issue_user_jwt` | 盖章 `purpose=user_login`（函数内常量；保留键加 purpose；后写覆盖） |
| `issue_access_token` | 新增参数 `purpose: &str`；先合并 extra_claims 后写 `purpose` |
| `issue_refresh_token` | 新增参数 `purpose: &str`；写入 refresh token custom（**供刷新继承**） |
| `issue_resource_token` | 盖章 `purpose=resource`（函数内常量） |
| service_jwt.rs | claims map（service_jwt.rs:62-70）加 `"purpose": "service"` |

调用侧：
- access.rs `handle_client_credentials` → `purpose="api_key"`；
- access.rs `handle_authorization_code` → access/refresh 均 `purpose="auth_code"`；
- refresh.rs `handle_refresh` → 从解码后的 refresh token custom 读 `purpose`
  透传给 `issue_access_token`；**旧 refresh token 无该键 → 新 access token 不带
  purpose**（与 §4.6 兼容语义一致，不猜测）。

### 4.5 签名体系与 JWKS：不变

RS256、kid=`default`、`/.well-known/jwks.json`（:8082 口径）全部不动
（硬性安全要求 2）。payload 加一个 custom 键对验签方透明。

### 4.6 兼容与切换策略（硬性安全要求 3）

- **claim 存在 ⇒ 权威**；**claim 缺失 ⇒ 存量旧令牌**，下游沿用现行
  aud/client_id 推断作为 fallback（一般 API 不受影响）。
- **A* 类 fail-closed 端点**（如需求修订 approve）：判据为
  `purpose == "user_login" && aud 存在`；`purpose` 缺失一律拒绝——即存量令牌
  自然淘汰（access token TTL 默认 1h，见 config default 3600s），持旧令牌用户
  重新登录即换新。**不设宽限期开关**：宽限窗等于把「无法区分」合法化，与 A*
  fail-closed 语义冲突；切换成本 = 一次集中重登录，且 token-service 重启本就
  换签名密钥、旧 JWT 全失效（下游已知特性），部署即完成切换。
- 本仓不做存量令牌重签（无状态 JWT，无重签设施；也不该有——重签等于伪造签发通道）。

### 4.7 对下游契约段落（可直接摘去 ai-pm 评审）

> **契约：JWT `purpose` claim（token-service 签发方声明）**
>
> 1. token-service 自本版本起在全部新签发 JWT 的 payload 写入字符串 claim
>    `purpose`，值域封闭枚举：`user_login`（登录/iam 刷新签发）、`api_key`
>    （API Key client_credentials 兑换）、`auth_code`（authorization_code 兑换）、
>    `service`（服务间 service-jwt）、`resource`（资源受限令牌）。
> 2. `purpose` 由**签发通道**决定，客户端任何请求均不可指定；存在即为签发方
>    权威声明，取代一切推断。
> 3. 存量令牌无该 claim：一般接口可按旧推断（`aud` 存在→interactive，
>    `client_id` 存在→api_key）兼容消费；**fail-closed 场景（如 A* 正式确认）
>    对 claim 缺失一律拒绝**，不设宽限。
> 4. 未知枚举值按非法处理（fail-closed）。
> 5. A* 判据采纳建议：`purpose == "user_login" && aud 存在` ⇒ 人类交互会话；
>    其余（api_key/auth_code/service/resource/缺失/未知）⇒ 非人类或不可判，
>    拒绝正式确认。`aud` 在新令牌中恒存在（issue_user_jwt 固定
>    `coder-micro-services`），保留 aud 检查为双保险，语义上以 purpose 为准。
> 6. **语义边界（必读）**：`purpose` 声明的是**签发通道**，不是「当前持有人
>    身份」。`purpose=user_login` 表示该令牌签发自一次真人密码登录，**不证明
>    当前持有者就是那位真人**——令牌被复制给 Agent 使用（如粘进 MCP 配置）
>    时 purpose 不变。因此本 claim 的防线成立依赖运行面配套（§4.9）：
>    机器接入必须走 `api_key` 通道，下游 runbook 不得再引导用登录令牌。
>    登录令牌被窃取/重放属另一威胁类（bearer 令牌固有），由短 TTL（默认 1h）、
>    `/tokens/revoke` 撤销、审计追溯治理，本 claim 不覆盖。
> 7. 消费无需 SDK 升级：claim 在 JWT payload，pm-api 现行 claims 解析
>    （serde_json 通用读取，auth.rs `AuthenticatedIdentity::parse`）加一个键即可；
>    token-service 镜像升级后立即可读。
> 8. 刷新继承：OAuth refresh 通道的新 access token 继承原 refresh token 的
>    purpose；iam refresh 与登录同走 `user_login`。`api_key` 通道无 refresh
>    （重兑换即续期）。

### 4.8 本仓校验点（token-service 侧消费方）

lmms 内部服务（jwt-ctx/authz 中间件）现行只校验签名/aud/roles，不读 purpose，
**零改动**。purpose 是否进 `jwt-ctx` 提取器属可选增强，非本稿范围（YAGNI，
等出现第二个消费方再做）。

### 4.9 A* 生效前提（运行面配套）与残余风险（builder check Q1 收编）

**purpose 只解决「通道可区分」，不解决「持有人是谁」。** 两者必须分开：

1. **通道分离（本稿解决）**：机器接入（MCP/AI 工具）从登录令牌迁移到
   `api_key` 通道（client_credentials）。该通道上游**已具备全部要件**：
   API Key 绑定 user_id（api_key.rs:24-29）、client_credentials 兑换时
   sub=绑定用户、权限交集、claims 拉取 fail-closed（access.rs:112-199）。
   迁移动作在下游 runbook/部署面：MCP 配置改用 API Key 兑换端点，登录令牌
   不再供机器使用。**此前**，下游 A* 的 `purpose==user_login` 判据对
   「登录令牌粘进 MCP」场景是 fail-open 的（该令牌天生带 user_login）——
   判据生效以迁移完成为前提，下游评审时须把这一前提写进 A* 的生效条件。
2. **持有人证明（本稿不解决）**：登录令牌被窃取/转交后的重放，是一切
   bearer 令牌的固有残余，任何签发侧 claim 都无法区分。治理手段 =
   短 TTL + 撤销 + 审计；若下游需要「确认动作发生时真人在场」的强证明，
   即下游备选方案 step-up 确认票据的领地（见 §6 比选——两案互补而非互斥）。

## 5. 目标 / 非目标 / 反目标

- **目标**：签发通道级用途声明；防客户端自报；向后兼容有明确语义；下游零 SDK
  成本消费。
- **非目标**：不改签名/JWKS/密钥管理；不做存量重签；不做 SDK/npm 发版；
  不改 iam-service；不引入令牌版本号体系。
- **反目标**：任何允许请求方影响 purpose 的设计（含内部调用方透传——保留键
  过滤拦住）；宽限期开关（把不可区分合法化）；用布尔/位标志压缩值域
  （封闭枚举的可扩展性与可审计性优先）。

## 6. 影响面 / 发布路径 / 工作量

**影响面**：仅 token-service 一个 crate。改动文件：issuer.rs（4 函数签名+盖章）、
access.rs（两 handler 传参）、refresh.rs（继承读取）、service_jwt.rs（claims map）、
tests（issuer_test.rs 新增用例 + integration 断言 purpose）。无 DB 迁移、无配置项、
无 SDK/npm 发版、无 iam/其他服务改动。

**发布路径**（既有协议）：lmms main → CI 镜像（MySQL/PG 双方言 tag）→ registry
→ 服务器栈与 ai-pm 测试栈同批重建。注意：token-service 重启换签名密钥 →
全量重登录，本就是已知特性，与 §4.6 切换策略天然对齐。

**工作量估计**：编码+单测 0.5 天；双方言容器测试串行跑 + 修 0.5 天；
文档（FEATURES.md 端点表顺手勘误）与发版 0.5 天。合计 **1.5~2 人日**。

**与下游备选（pm-api 自建 step-up 确认票据）比选（builder check 后修正口径）**：

| 维度 | purpose claim（本稿） | pm-api step-up 票据 |
|---|---|---|
| 跨仓 | 是（本仓改，下游读） | 否（下游自闭环） |
| 防线位置 | 签发级——**通道分离** | 确认动作级——**在场证明** |
| 能否防登录令牌重放 | **不能**（§4.9-2，bearer 固有） | **能**（确认时须再出一张即时票据） |
| 收益面 | 所有多租户服务受益，一次建成 | 仅 ai-pm 受益 |
| 周期 | 1.5~2 人日（含发版） | 下游自估 |
| 耦合 | 下游依赖上游发版节奏；A* 生效还需 runbook 迁移配套（§4.9-1） | 无跨仓依赖 |
| 残余风险 | 运行面迁移完成前判据 fail-open | 票据签发/校验自成体系，多一套要维护的面 |

**结论：两案互补，非互斥。** purpose 解决「机器通道与人类通道可区分」（含
给 MCP 正确的接入方式、下游可拒绝非 user_login 令牌），step-up 解决「确认
瞬间真人在场」。若下游 A* 的威胁模型只关心通道区分，purpose 足够；若必须
防凭据重放，step-up 不可省。purpose 落地不影响未来再加 step-up。

## 7. 验收清单（本稿实施时）

1. 五通道新签发令牌 payload 均含正确 purpose（按 §4.2 矩阵逐一断言）；
2. `UserJwtRequest.claims` 里塞 `purpose` 被丢弃（保留键过滤用例）；
3. iam claims 端点返回体若含 `purpose` 键不被采纳（后写覆盖用例）；
4. OAuth refresh：新 refresh token 含 purpose，刷新出的 access token 继承；
   旧格式 refresh token（无 purpose）刷新出的 access token 无 purpose；
5. client_credentials 无绑定 user_id 的 key 兑换仍 400（现状回归）；
6. JWKS/签名/kid 不变（现行 verifier 全部照常通过）；
7. MySQL/PG 双方言测试均绿（串行约束）。

## 8. 相邻发现（不在本稿范围，建议另立小项）

1. **authorization_code fail-open**：iam 不可达时兜底签
   `roles:["admin"] + permissions:["notification.admin"]`（access.rs:235-239），
   与 client_credentials 的 fail-closed（access.rs:164-171）及 refresh 的 503
   （refresh.rs:126-133）不一致。建议改为 fail-closed（对齐 503），一小改动，
   但属安全语义变更，单独评审。
2. `ServiceJwtCache::sign_local`/`prime_cache`（§2.4 例外段）：现行仅测试调用、
   不构成越权，但属「测试设施住在生产 lib 里」。候选清理：搬入 test-util
   crate；涉及 common-jwt 发版，单独立项评估。
3. `issue_access_token` 合并覆盖前置键（issuer.rs:49-63）已由 §4.3-2 顺带硬化，
   不单列。
4. FEATURES.md API 端点表路径陈旧（§3 附注），实施时顺手勘误。
