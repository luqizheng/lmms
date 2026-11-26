# WP-LMMS-HARDEN-01-R1 · Resource Token 专项分析

- 审计对象：`lmms` @ `main` = `92691f3313800cc973451e7ca8259753cda666f0`
- 审计日期：2026-11-26
- 裁决速览：**设计意图 = KEEP 的受控资源凭证；实际实现 = FIX（公开签名神谕 + 孤儿令牌）——本包已完成最小修复（认证门禁），令牌契约本身留待提案**

---

## 1. 接口原始设计目的（文档 + 代码交叉）

- 设计规范 `docs/superpowers/specs/2025-07-11-token-server-api-gateway-design.md:349-367`：签发**单一资源、单一权限、短期（约 5 分钟）** 的受限 JWT，claims 示例 `sub=<用户ID>`、`resource_type=contract`、`permission=read`；**:415 端点表标注「JWT 验证 = 是」**。
- `docs/superpowers/specs/2026-11-04-token-purpose-claim-design.md`：`purpose=resource` 通道定义"资源受限令牌"。
- `docs-site/guides/token-service-overview.md:36`（修复前）：鉴权列标注 **`public`**——**与设计规范自相矛盾**（修复后已同步为 service-jwt）。
- Java SDK 双端（`digua-token-client-sdk` / `digua-token-user-sdk`）均有 `issueResource(resourceType, resourceId, permission, ttlSec)` 封装；Rust SDK 未封装。

**原始定位**：给资源服务做细粒度临时授权的基础设施凭证——业务系统做业务授权后，由受控调用方为特定资源申请短时令牌。

## 2. 任务书十二问（修复前实际实现，全部代码证实）

| # | 问题 | 实际答案 | 证据 |
|---|---|---|---|
| 1 | 签发什么 Token | RS256 JWT（jwt-simple，带 keystore kid header） | `issuer.rs:241-280` |
| 2 | Resource 指什么 | **通用资源**——调用方自填的 `resource_type`+`resource_id` 字符串对，不校验存在性/归属 | `routes/resource.rs:7-13` |
| 3 | 为用户还是服务代表用户申请 | **都不是**——`sub` 恒为硬编码 `"resource-service"`，调用方身份完全不进 token | `resource.rs:30` |
| 4 | 一个 Token 能否访问多个资源 | 不能（单资源绑定，单值 claim） | `issuer.rs:260-267` |
| 5 | 读/写/删权限范围 | 形式上有单字符串 `permission` claim，**无值域校验**（任意字符串接受） | `issuer.rs:268-271` |
| 6 | 绑定租户？ | **否**（无 tid） | claims 清单 |
| 7 | 绑定调用方？ | **否**（无调用方身份；端点本身也无认证） | `main.rs:188-191` |
| 8 | 绑定目标服务/audience？ | **否**（无 aud） | 同上 |
| 9 | 短有效期？ | 默认 300s（`config.yaml:36`），**可按请求覆盖且无上限封顶** | `resource.rs:25` |
| 10 | 支持撤销？ | 形式上可经 `/tokens/revoke` 按 jti 写 DB 黑名单，**但无任何执行点读取**（`revoke_svc::is_revoked` 零调用方；网关只查 Redis）→ 实际不可撤销 | `revoke_svc.rs:23-30` |
| 11 | 资源服务如何验证 | **【未找到】全仓无任何验证实现**；file-service 因无 aud 拒绝（`AudienceMismatch` 401），网关保护路由因无 tid 拒绝 | `file-service/routes/mod.rs:33-39`、`middleware/mod.rs:274-277` |
| 12 | 能否跨资源/租户/服务使用 | 不能跨资源；**一旦被任何验证方接受即天然跨租户跨服务**（无任何绑定约束） | — |

## 3. 完整申请流程还原（§4.2）

**设计意图的流程**：
```
用户 →（登录 JWT 证明身份）→ 业务系统 →（业务授权判断）
   →（服务身份 + 受约束的资源请求）→ LMMS /tokens/resource
   → 资源服务（验证 token：签名/aud/租户/资源/权限/过期）→ 放行
```

**修复前的实际流程**【代码证实】：
```
任意网络调用方 →（无任何凭据）→ 网关 public 白名单直通
   → token-service 原样签发（sub=resource-service，三元组任意，TTL 任意）
   → 没有资源服务会验证它（孤儿令牌，终点是调用方自己的口袋）
```

用户身份、业务授权、服务身份三个环节**全部缺失**。实际实现与目标设计的差距不是"实现不完整"，而是"信任链每一环都没接"。

## 4. 业务授权边界（§4.3 A/B/C 拆分）

| 层 | 问题 | 修复前结论 |
|---|---|---|
| A 业务系统授权决策 | — | 不适用：流程中根本没有业务系统参与 |
| B LMMS 验证申请方与基础设施访问范围 | 是否验证申请方身份？ | **否**（匿名可签发）→ **P0 缺陷**（本包已修） |
| B 附加 | 是否允许匿名/伪造/跨租户/范围扩大？ | 全部允许（输入零验证）→ 随认证修复而消除入口，输入校验留作加固建议 |
| C 资源服务执行限制 | 是否验证 token？ | **否（无实现）**——当前不存在被绕过的资源面；未来对接前必须建验证端 |

## 5. 安全属性核对（§4.4，修复前）

| 属性 | 状态 |
|---|---|
| issuer | ✓ 有（配置值） |
| subject/调用方身份 | ✗ 硬编码常量，无调用方绑定 |
| audience | ✗ 无 |
| tenant | ✗ 无 |
| resource scope | ✓ 有（resource_type+resource_id，但不校验） |
| permission/action | ✓ 有（单字符串，无值域） |
| iat | ✓（jwt-simple 盖章） |
| expires_at | ✓ 有 TTL，但无上限 |
| jti | ✓ 有 UUID |
| 不可自行扩大权限 | ✗（三元组任意自填） |
| 过期拒绝 | N/A（无验证方） |
| 重放 | N/A |
| 撤销 | ✗（黑名单无执行点） |
| fail-closed | ✗（无认证=全放行） |

## 6. 指控 1 的最终裁决与修复

**裁决：指控成立【代码证实】**——`/tokens/resource` 是签发类端点中唯一无认证的（对比：`/tokens/user-jwt` 有 service-jwt 门禁、`/tokens/service-jwt` 有 bootstrap 校验、`/tokens/access` 有 client 凭据校验、`/admin/*` 全部有门禁），叠加网关 `public=true`，构成**公开签名神谕**：任意人可无限量获取由平台可信密钥签名的、声明任意的合法 JWT。缓解因素是当前零消费方（直接利用面有限），但该神谕本身就是未来任何 token 验证逻辑的定时炸弹，且与设计规范「JWT 验证=是」相悖。

**修复决策：FIX（保留端点、补认证），不 DEPRECATE**——依据：Java SDK 契约仍在、任务书 §4.5 禁止仅因无消费者即删、设计意图（受控资源凭证）本身合理。

### 修复内容（最小变更，5 个文件 + 1 个新测试）

| 文件 | 变更 |
|---|---|
| `services/token-service/src/routes/resource.rs` | 新增 `routes(state)` 构造器，挂 `verify_aud_principal_middleware("token-service")`（镜像 `keys_admin.rs:287` 模式） |
| `services/token-service/src/main.rs` | 裸路由移出主 Router，改为 `app.merge(routes::resource::routes(state.clone()))`（与 `/tokens/user-jwt` 同层级、同注释风格） |
| `examples/docker-deploy/config/api-gateway/routes/token.router.toml` | `/v1/tokens/resource` 删除 `public = true` + 注释说明 |
| `examples/docker-deploy-full/config/api-gateway/routes/token.router.toml` | 同上（两副本已 diff 确认一致） |
| `docs-site/guides/token-service-overview.md` + `services/token-service/{AGENTS,README,FEATURES}.md` | 鉴权描述 public → service-jwt（4 处） |
| `services/token-service/tests/resource_token_auth_test.rs`（新增） | 3 例回归：无认证 401 / 错 aud service-jwt 401 / 正确 aud 签发且 claims 逐字段不变 |

**明确不做（留待提案）**：token claims 增加 aud/tid/调用方绑定（契约变更）；TTL 上限封顶；permission 值域枚举；撤销链路接通（DB 黑名单 → 网关/验证方读取）。这些属于 Resource Token 的 REDEFINE 设计提案范围，须人工批准后另立工作包。

## 7. 测试证据

- 命令：`TEST_MYSQL_HOST=127.0.0.1 TEST_MYSQL_PORT=13306 cargo test -p token_service`（共享 MySQL 8.0.46；详见 AUDIT-REPORT 环境备注）
- 结果：**170 passed / 0 failed**（lib 79 + 集成 92 目标中 16 个可运行目标共 91 例 + 新增 3 例计入其中）；`pg_roundtrip_test` 在共享 MySQL 模式下按设计执行 0 例。
- 新增测试单独确认：`cargo test -p token_service --test resource_token_auth_test` → **3 passed / 0 failed**。
- 修复有效性由负面测试锁定：两条 401 用例在修复前必然 200（端点当时无认证）——即"未认证申请被拒"的行为从未存在过测试，现在是回归锁定的契约。
- fmt/clippy：`cargo fmt -p token_service` ✓；`cargo clippy -p token_service --all-targets` 无新增警告（既有的 `digua_common` 未使用依赖警告系历史状态：`ResourceTokenClaims` 定义于 libs/common 但全仓零使用， removal 属契约裁决范围）。

## 8. 最终裁决

| 项 | 结论 |
|---|---|
| Resource Token 设计合理性 | **FIX**（已修：认证补齐；契约语义维持现状） |
| Resource Token 安全性 | **PASS（修复后）**——签名神谕入口关闭；令牌自身仍无 aud/tid/撤销，**不得在任何验证方实现前投入使用** |
| 遗留建议 | 若未来启用：按 REDEFINE 提案补 audience/租户/调用方绑定 + 撤销执行点 + TTL 上限 + permission 值域；否则列入 DEPRECATE 计划（连同 Java SDK `issueResource` 与 `ResourceTokenClaims` 死代码） |
