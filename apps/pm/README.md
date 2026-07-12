# PM Platform

禅道（Zentao）替代的内部研发管理工具：项目 / 需求 / 用户故事 / 验收标准 + 知识库（空间 / 文档 / 版本 / Markdown）+ LLM 接口预留。

> ⚠️ **MVP 状态（v0.1.0）**：
> - ✅ 骨架：6 个 POM（parent + 5 子模块）、Spring Boot 启动、Flyway V1（7 表）、健康检查、JWT 鉴权、CORS、LlmProvider 接口 + NoopLlmProvider、Dockerfile。
> - ✅ 业务实现 v1：项目管理（project / requirement / user_story / acceptance_criteria）+ 知识库（space / document / version）+ Markdown 渲染（flexmark-java）。
> - 🚫 未启用：真实 LLM（接口已留，等接入火山方舟/DeepSeek/Ollama）；OAuth/OIDC 与 IAM 互通；任务/缺陷/用例/迭代管理。

## 模块结构

```
apps/pm/
├── pm-app-bootstrap/        @SpringBootApplication 入口（健康检查 + Security + JWT filter）
├── pm-app-api/              REST controllers（Project/Requirement/Story/AC + KB）
├── pm-app-domain/           领域模型 + 业务规则 + 共享异常（NotFound/Conflict）
├── pm-app-infrastructure/   MyBatis-Plus + Flyway + SDK 集成 + Markdown 渲染
└── pm-app-llm/              LlmProvider 抽象 + NoopLlmProvider 默认实现
```

## SDK 集成（low-memory-micro-services 互通）

`pm-app-infrastructure` 提供 1 个 SDK 配置 + 1 个上报包装 + 6 个 service 的真实接入：

| 类 | 类型 | 作用 |
|---|---|---|
| `SdkConfig` | `@Configuration` | `@Bean` 产出 4 个 SDK client：`AuditServiceClient` / `FileServiceClient` / `NotificationServiceClient` / `ConfigServiceClient`（从 `application.yml` 的 `pm.sdk.*` 读 base-url + bearer） |
| `AuditEventReporter` | `@Component` | 包装 `AuditServiceClient`，暴露 `report(actor, action, resourceType, resourceId, payload)`；SDK 异常**仅 warn**，不阻塞业务（MVP 原则） |
| 6 个 service 内的 16 处 audit.report(...) | 真实使用 | 详见下表 |

**Audit 事件 taxonomy**（action / 资源类型 / 触发点）：

| Service | Method | Action | Resource type |
|---|---|---|---|
| `ProjectService` | `create / update / archive` | `PM_PROJECT_CREATED / UPDATED / ARCHIVED` | `pm_project` |
| `RequirementService` | `create / update` | `PM_REQUIREMENT_CREATED / UPDATED` | `pm_requirement` |
| `UserStoryService` | `create / update` | `PM_USER_STORY_CREATED / UPDATED` | `pm_user_story` |
| `AcceptanceCriteriaService` | `create / update / check(true/false)` | `PM_AC_CREATED / UPDATED / CHECKED / UNCHECKED` | `pm_acceptance_criterion` |
| `KbSpaceService` | `create / update` | `KB_SPACE_CREATED / UPDATED` | `kb_space` |
| `KbDocumentService` | `create / updateMetadata / updateContent / softDelete` | `KB_DOCUMENT_CREATED / METADATA_UPDATED / CONTENT_UPDATED / DELETED` | `kb_document` |

actor 全部取自 `SecurityContextHolder`（无认证时 fallback `"system"`）。

Endpoints 由 `application.yml` 的 `pm.sdk.{audit,file,notification,config}.base-url` 控制（默认 `http://localhost:3000` 即 api-gateway）。bearer 默认取 `pm.sdk.bearer.static-token`（默认 `dev-bypass-token`）。

后端单元测试（`pm-app-infrastructure/src/test/.../AuditEventReporterTest.java`，JUnit 5）：
- 验证 report 调用 SDK 透传正确的 Actor/Action/ResourceType/ResourceId/Payload
- 验证 SDK 抛运行时异常时 reporter 不向上抛
- 验证 null payload 也能消化

后续 PR 接入点（不在本 MVP）：
1. `RequirementService` / `KbSpaceService` / `KbDocumentService` 等 5 个 service 按 `ProjectService` 同样模式加 audit
2. `KbDocumentService.updateContent` 附件上传走 `FileServiceClient.uploadFile(...)`
3. 评论 / @提醒模块调 `NotificationServiceClient.createAnnouncement(...)`
4. LLM provider 选择走 `ConfigServiceClient.getConfigItem(...)` 读运行时配置

**当前 status**：wiring seam + ProjectService 真实接入 + AuditEventReporter 单测 ✅。阻塞剩余项：① SDK 三个新模块发布到 `mvn.kudi.repos`；② Docker compose up 让 SDK 真实调用能通；③ 其他 service 接入审计。

## 端点

### System

| Method | Path | 说明 |
|---|---|---|
| `GET` | `/pm-api/v1/health` | liveness（不依赖 DB） |
| `GET` | `/pm-api/v1/health/ready` | readiness（MySQL ping） |
| `GET` | `/pm-api/v3/api-docs` | OpenAPI JSON |
| `GET` | `/pm-api/swagger-ui/index.html` | Swagger UI |

### Project Management（PM）

| Method | Path | 说明 |
|---|---|---|
| `POST` | `/pm-api/v1/projects` | 创建项目 |
| `GET` | `/pm-api/v1/projects?page=&size=&status=` | 分页列表 |
| `GET` | `/pm-api/v1/projects/{id}` | 项目详情 |
| `PATCH` | `/pm-api/v1/projects/{id}` | 更新项目 |
| `DELETE` | `/pm-api/v1/projects/{id}` | 归档项目 |
| `POST` | `/pm-api/v1/projects/{id}/requirements` | 创建需求 |
| `GET` | `/pm-api/v1/projects/{id}/requirements?status=&type=` | 项目下需求列表 |
| `GET` | `/pm-api/v1/requirements/{id}` | 需求详情 |
| `PATCH` | `/pm-api/v1/requirements/{id}` | 更新需求 |
| `DELETE` | `/pm-api/v1/requirements/{id}` | 删除需求 |
| `POST` | `/pm-api/v1/requirements/{id}/stories` | 追加用户故事 |
| `GET` | `/pm-api/v1/requirements/{id}/stories` | 故事列表 |
| `GET` | `/pm-api/v1/stories/{id}` | 故事详情 |
| `PATCH` | `/pm-api/v1/stories/{id}` | 更新故事 |
| `POST` | `/pm-api/v1/stories/{id}/criteria` | 追加验收标准 |
| `GET` | `/pm-api/v1/stories/{id}/criteria` | 验收标准列表 |
| `PATCH` | `/pm-api/v1/criteria/{id}` | 更新验收标准 |
| `POST` | `/pm-api/v1/criteria/{id}/check` | 勾选/取消验收 |

### Knowledge Base（KB）

| Method | Path | 说明 |
|---|---|---|
| `POST` | `/pm-api/v1/spaces` | 创建知识库空间 |
| `GET` | `/pm-api/v1/spaces?page=&size=&projectId=&visibility=` | 分页列表 |
| `GET` | `/pm-api/v1/spaces/{id}` | 空间详情 |
| `PATCH` | `/pm-api/v1/spaces/{id}` | 更新空间 |
| `DELETE` | `/pm-api/v1/spaces/{id}` | 删除空间 |
| `POST` | `/pm-api/v1/spaces/{id}/documents` | 创建文档（含 markdown v1） |
| `GET` | `/pm-api/v1/spaces/{id}/documents?parentId=` | 空间下文档列表（树形） |
| `GET` | `/pm-api/v1/documents/{id}` | 文档详情 |
| `PATCH` | `/pm-api/v1/documents/{id}` | 更新文档元数据 |
| `POST` | `/pm-api/v1/documents/{id}/content` | 更新内容（产生新版本） |
| `DELETE` | `/pm-api/v1/documents/{id}` | 删除文档 |
| `GET` | `/pm-api/v1/documents/{id}/versions` | 版本列表（最新优先） |
| `GET` | `/pm-api/v1/documents/{id}/versions/latest` | 最新版本 |
| `GET` | `/pm-api/v1/documents/{id}/versions/{n}` | 指定版本 |
| `POST` | `/pm-api/v1/documents/{id}/rollback/{n}` | 回滚到版本 n（产生新版本） |

### LLM（占位）

| Method | Path | 说明 |
|---|---|---|
| `GET` | `/pm-api/v1/llm/providers` | 🔜 PR-LLM（返回当前可用 provider 名） |
| `POST` | `/pm-api/v1/llm/requirements/{id}/decompose` | 🔜 拆解需求为故事 + AC（Noop 暂只返回 disabled 响应） |

## 快速开始

### 本机编译（需要 JDK 17/21 + Maven 3.9+）

```bash
cd apps/pm
mvn -Pjava17 clean package -DskipTests
java -jar pm-app-bootstrap/target/pm-app-bootstrap.jar \
     --spring.profiles.active=dev
```

### Docker

```bash
# 默认端口 8088，需要 MySQL（在 host 网络下）
docker build -t pm-platform:0.1.0 -f apps/pm/Dockerfile .
docker run --rm -p 8088:8088 \
  -e SPRING_DATASOURCE_URL='jdbc:mysql://host.docker.internal:3306/pm_platform?useSSL=false&serverTimezone=UTC&allowPublicKeyRetrieval=true' \
  -e LOWMEM_GATEWAY_BASE='http://api-gateway:3000' \
  -e PM_JWT_JWKS_URL='http://api-gateway:3000/v1/tokens/.well-known/jwks.json' \
  pm-platform:0.1.0

# 启动后验证
curl http://localhost:8088/pm-api/v1/health
curl http://localhost:8088/pm-api/v1/health/ready
```

### docker-compose

`infrastructure/docker-compose.yml` 已添加 `pm` 服务；`up -d pm` 启动。

## 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `SPRING_PROFILES_ACTIVE` | `dev` | Spring profile |
| `SERVER_PORT` | `8088` | HTTP 端口 |
| `SPRING_DATASOURCE_URL` | `jdbc:mysql://localhost:3306/pm_platform?...` | MySQL JDBC URL（独立 schema `pm_platform`，与 Rust 服务同 MySQL 实例） |
| `SPRING_DATASOURCE_USERNAME` | `root` | DB 用户 |
| `SPRING_DATASOURCE_PASSWORD` | `rootpass` | DB 密码 |
| `PM_JWT_MODE` | `jwks`（生产）/ `noop`（dev profile 默认） | JWT 验签模式：`jwks`（RS256 + iam-service JWKS）/ `hs256`（HS256 dev fallback）/ `noop`（仅 `dev` profile，跳过 token 校验） |
| `PM_JWT_JWKS_URL` | `http://localhost:3000/v1/tokens/.well-known/jwks.json` | JWKS endpoint（生产指向 api-gateway / token-service） |
| `PM_JWT_ISSUER` | `superpowers-token-service` | 期望的 JWT issuer |
| `PM_JWT_AUDIENCE` | `coder-micro-services` | 期望的 JWT aud |
| `PM_JWT_REQUIRED_SCOPE` | `access` | 期望的 JWT scope（拒绝 `refresh` / `resource`） |
| `PM_JWT_CACHE_TTL_MINUTES` | `10` | JWKS 公钥缓存 TTL |
| `PM_JWT_HMAC_SECRET` | *(empty)* | HS256 dev 模式共享密钥（生产禁止设置） |
| `PM_LLM_PROVIDER` | `noop` | 默认 LLM provider 名（`noop` / `deepseek`） |
| `DEEPSEEK_API_KEY` | *(empty)* | DeepSeek API Key；非空时自动启用 DeepSeek provider |
| `DEEPSEEK_BASE_URL` | `https://api.deepseek.com` | DeepSeek Chat Completions base URL（兼容 OpenAI 协议的服务都可替换） |
| `DEEPSEEK_MODEL` | `deepseek-v4-flash` | DeepSeek 模型名 |
| `LOWMEM_GATEWAY_BASE` | `http://localhost:3000` | api-gateway baseURL（SDK 调用） |
| `PM_SDK_BEARER` | `dev-bypass-token` | SDK 调用时用的静默 bearer |

### 启用真实 LLM（DeepSeek）

默认情况下 apps/pm 启动时注册 `noop` 和 `deepseek` 两个 provider，但 `deepseek` 在未配置 API Key 时 `isEnabled()=false`，`registry.dispatch("deepseek", …)` 会自动回退到 noop。启用只需：

```bash
export DEEPSEEK_API_KEY=sk-your-real-key
export PM_LLM_PROVIDER=deepseek   # 可选：把默认 provider 切成 deepseek
java -jar pm-app-bootstrap/target/pm-app-bootstrap.jar
```

也可在 `application.yml` 写死：

```yaml
pm:
  llm:
    provider:
      default: deepseek
    deepseek:
      api-key: sk-...
      base-url: https://api.deepseek.com
      model: deepseek-v4-flash
```

前端需求详情页的「AI 拆解」会把需求发给 DeepSeek，返回 3–7 个用户故事 + 验收标准的 JSON，用户确认后可一键批量导入。「AI 改写」则返回重写后的描述，支持「应用改写」一键覆盖。

## 与 low-memory-micro-services 的关系

```
Browser → vue-pure-admin → /pm-api → apps/pm (Spring Boot, 8088)
                                       │
                                       ├─ 直连 MySQL `superpowers` 实例，独立 schema `pm_platform`（pm_/kb_ 表）
                                       └─ SDK → api-gateway (:3000) → Rust 服务
                                                  ├─ audit-service-sdk  → audit-service
                                                  ├─ file-service-sdk   → file-service
                                                  ├─ notification-service-sdk → notification-service
                                                  └─ config-service-sdk → config-service
```

- **共享 MySQL 实例，独立 schema**：apps/pm 与 Rust 服务共用同一 MySQL 实例（`superpowers`），但 PM 表放独立 schema `pm_platform`（pm_/kb_ 前缀，已在 `scripts/check-db-naming.js` 白名单）。JDBC URL 必须指向 `pm_platform`；Flyway 自动建表。
- **跨服务用户引用**：`owner_id` / `assignee_id` / `created_by` 存 BIGINT，**无物理外键**（iam-service 是独立服务）；写入前调 `iam-service` 校验存在性。
- **JWT**（Phase B）：apps/pm 通过 `JwksJwtVerifier` 验证 iam-service / token-service 签发的 RS256 JWT；issuer=audience=scope 与 `services/token-service/src/issuer.rs::issue_user_jwt` 对齐。从 JWT 抽 `sub`(UUID)→`userId`(long)、`user_code`→`username`、`tid`→`tenantId`、`roles[]` / `permissions[]` / `scope`，注入 `Principal` 给 `Authentication.principal`，控制器可用 `@AuthenticationPrincipal Principal p` 取。
- **域 service actor 读取**（Phase F1 修复）：`com.coder.pm.infra.security.CurrentActor` 工具集中读 SecurityContext 内的 `Principal`，6 个 service（Project / Requirement / UserStory / KbSpace / KbDocument / AcceptanceCriteria）的 `create` 在 caller 没传 `createdBy` 时自动 fallback 到当前 actor id；audit `report(...)` 用 `displayName()`。Dev profile + NoOpJwtVerifier 下自动用 userId=1，所以 `POST /v1/projects`、`/requirements`、`/stories`、`/spaces`、`/documents` 在 dev 都不再报 500。
- **端到端联调**：`POST {LOWMEM_GATEWAY_BASE}/v1/iam/auth/login` 拿 token → `Authorization: Bearer <jwt>` 调 `/pm-api/v1/llm/providers` / `/pm-api/v1/projects` → 200 OK；伪造 / 错签名 / 无 token → 403 Forbidden；健康检查 `/pm-api/v1/health` 公开。
- **Dev profile bypass**（Phase F1）：`SPRING_PROFILES_ACTIVE=dev` 时 `JwtSecurityConfig` 走 `Development` 子配置（`@Profile("dev")`），注入 `NoOpJwtVerifier`，对任意请求（含无 token / 错 token / 真 token）都返回固定的 dev `Principal`（userId=1, username=`dev`, roles=`[pm_admin]`）。本地开发不再需要先登录 iam-service 也能 curl/前端/e2e 跑通；`pm.jwt.mode=noop` 仅在 `application-dev.yml` 默认生效，prod 配置文件仍是 `jwks`，**不会**误启用 bypass。

## nginx 接入

```nginx
location /pm-api/ {
    proxy_pass http://pm:8088/pm-api/;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

## 文档

- 设计 spec：`docs/superpowers/specs/2026-07-11-pm-platform-design.md`
- 服务规范：本目录 `AGENTS.md`

## CI / 本地检查

- `mvn -Pjava17 verify` — 编译 + 测试 + 打包
- `docker build` 通过 Dockerfile 构建
- 表前缀检查：`node scripts/check-db-naming.js`（已加 pm_/kb_ 白名单）

## 后续 PR（不属 MVP）

- PR-2：B1 业务模块（Project / Requirement / UserStory / AC）
- PR-3：B2 业务模块（KB Space / Document / Version + Markdown 渲染）
- PR-4：LLM 接口（拆解需求 API + 完成切真实 provider）
- PR-5：Spring Security + JWT 完整身份验证（user 表 + 登录）
- PR-6：vue-pure-admin 路由 + 视图
- PR-7：完善 e2e + 文档

## 端到端验证（Phase A + Phase B 实测）

**Phase F1（Dev profile JWT bypass）**：

`SPRING_PROFILES_ACTIVE=dev` 起 PM 容器后无需任何 token 即可调全部受保护端点：

```bash
# 无 token 直调 GET 受保护端点
curl -s http://localhost:8088/pm-api/v1/llm/providers
# → ["deepseek","noop"]  (HTTP 200)

curl -s http://localhost:8088/pm-api/v1/projects?page=1\&size=5
# → {"items":[...]}     (HTTP 200)

# 无 token POST 创建（dev principal userId=1 自动注入）
curl -s -X POST http://localhost:8088/pm-api/v1/projects \
  -H "Content-Type: application/json" \
  -d '{"code":"DEV-1","name":"Dev Test","ownerId":1}'
# → HTTP 201 + 项目 payload
```

`pm.jwt.mode=noop` 仅 dev profile 生效；prod 配置文件 (`application.yml`) 默认 `jwks`，JWT 验签逻辑不变。`JwtSecurityConfig.Production` 子配置没声明 `NoOpJwtVerifier` bean，prod 模式下不存在 bypass。

**Phase F2（Dev demo data seed）**：

```bash
# 起 PM dev profile 容器后，跑：
PM_BASE=http://localhost:8088 scripts/seed-pm.sh

# 幂等创建：1 个 DEMO 项目 + 2 个 requirement + 2 个 story + 2 条 AC + 1 个 KB space + 1 个 markdown doc
# 用 dev principal（Phase F1 自动启用），所以不需要 token
```

依赖 `python3` 解析响应（替代 jq）。每次跑都按 code/title 查重：已存在则跳过，缺失则补齐。

**Phase B（IAM JWT 互通）**：

```
# 1. 用 iam-service 登录拿真 token
curl -s -X POST http://localhost:3000/v1/iam/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","password":"..."}'

# 2. 调 PM 受保护端点
TOK=...
curl -s http://localhost:8088/pm-api/v1/llm/providers \
  -H "Authorization: Bearer $TOK"
# → ["deepseek","noop"]

# 3. 调 PM 列表
curl -s http://localhost:8088/pm-api/v1/projects \
  -H "Authorization: Bearer $TOK"
# → {"items":[...]}
```

✓ 真打 iam-service RS256 JWT 通过 PM JwtAuthFilter，密钥从 `LOWMEM_GATEWAY_BASE/v1/tokens/.well-known/jwks.json` 拉取并缓存。伪造 token / 改坏签名 → 403 Forbidden。

**Phase A（SDK 链路）**：容器启动后跑 `scripts/pm-smoke.sh` 触发的 service 层 audit 调用，pm 容器日志显示 SDK 真在打 audit-service：

```
WARN c.coder.pm.infra.sdk.AuditEventReporter : audit-service report failed
     (action=PM_PROJECT_CREATED,  resource=pm_project/2): audit 401: Invalid or expired token
```

**SDK 链 4 个链路（`ProjectService` × 3 + `KbSpaceService` × 1）已端到端打穿**：
- `service.create() → audit.report() → AuditEventReporter → AuditServiceClient → HTTP POST api-gateway:3000/v1/audit/events → audit-service`
- audit 401 是 SDK 调用时仍带 `PM_SDK_BEARER=dev-bypass-token`，不是 iam-service 发行的 JWT；这是 SDK 调用侧的 token 注入问题，独立于 PM 的 inbound JWT 验证。Phase G 计划用 service-to-service JWT 替换。

