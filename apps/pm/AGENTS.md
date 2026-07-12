# PM Platform — 服务规范

## 必需组件（8-item 服务规范）

- [x] Dockerfile — `apps/pm/Dockerfile`（multi-stage: `maven:3.9.9-eclipse-temurin-21` 构建 + `eclipse-temurin:21-jre-alpine` 运行时）
- [x] config/ — `apps/pm/config/{application.yml, application-dev.yml}` + `pm-app-bootstrap/src/main/resources/{application.yml, application-dev.yml}`
- [x] README.md — `apps/pm/README.md`
- [x] AGENTS.md — 本文件
- [x] Health endpoint — `/pm-api/v1/health`（liveness）+ `/pm-api/v1/health/ready`（readiness w/ DB ping）
- [x] Database migration — Flyway `pm-app-infrastructure/src/main/resources/db/migration/V1__init_*.sql`
- [x] Swagger/OpenAPI — springdoc-openapi 2.6.0, `/pm-api/v3/api-docs` + `/pm-api/swagger-ui/index.html`
- [x] Tests — 单测随业务 module 落地（PR-3/4 已计划）；MVP 阶段仅 compile + boot 验证。

## 配置选项

- `SPRING_PROFILES_ACTIVE`: dev / prod / staging
- `SERVER_PORT`: 默认 8088
- `SPRING_DATASOURCE_URL/USER/PASSWORD`: MySQL 连接（默认 `pm_platform` schema，与 Rust 服务的 `superpowers` 库同实例、独立 schema）
- `PM_LLM_PROVIDER`: LLM provider 名，默认 noop（noop / deepseek）
- `DEEPSEEK_API_KEY` / `DEEPSEEK_BASE_URL` / `DEEPSEEK_MODEL`: DeepSeek (OpenAI-compatible) 接入配置；api-key 非空时 deepseek provider 自动启用
- `PM_JWT_MODE`: `jwks`（默认，RS256 + 共享 iam-service JWKS）/ `hs256`（dev fallback，共享密钥 HS256）/ `noop`（仅 `dev` profile，跳过 token 校验）
- `PM_JWT_JWKS_URL`: JWKS endpoint（默认 `http://localhost:3000/v1/tokens/.well-known/jwks.json`）
- `PM_JWT_ISSUER`: 期望的 JWT issuer（默认 `superpowers-token-service`，与 iam-service 对齐）
- `PM_JWT_AUDIENCE`: 期望的 JWT aud（默认 `coder-micro-services`）
- `PM_JWT_REQUIRED_SCOPE`: 期望的 JWT scope（默认 `access`，拒绝 `refresh` / `resource`）
- `PM_JWT_CACHE_TTL_MINUTES`: JWKS key 缓存 TTL（默认 10 分钟）
- `PM_JWT_HMAC_SECRET`: HS256 dev 模式用的共享密钥（生产环境禁止设置）

### Dev profile JWT bypass

`SPRING_PROFILES_ACTIVE=dev` 时（默认）`JwtSecurityConfig` 走 `Development` 子配置：注入一个 `NoOpJwtVerifier`，对每个请求都返回固定的 dev `Principal`（userId=1, username=`dev`, tenantId=1, roles=`[pm_admin]`, permissions=`[pm:*]`, scope=`access`），使任何受保护端点都无需 token。

- 默认（`dev` profile + `pm.jwt.mode=noop`）：JWT 验签被完全跳过，本地 curl / 前端 / e2e 都无需登录。
- dev profile 想用真 JWKS 验证：`pm.jwt.mode=jwks` 即可覆盖，模式切换在 `JwtVerifierRouter` 内部完成。
- 生产 profile：自动注入 `JwksJwtVerifier`，配置 `pm.jwt.mode=jwks` / `hs256` 之一，**绝不**会启用 noop（`JwtSecurityConfig.Production` 没声明 NoOpJwtVerifier bean）。

`pm.jwt.mode: noop` 仅在 `dev` profile 配置文件 (`application-dev.yml`) 中默认启用；`application.yml` 的默认仍是 `jwks`，避免误用。
- `LOWMEM_GATEWAY_BASE`: api-gateway baseURL，SDK 内部使用
- `PM_SDK_BEARER`: SDK 内部用的静默 bearer（MVP 阶段）

## 项目结构

```
apps/pm/
├── pm-app-bootstrap/         # @SpringBootApplication + Security + JWT filter + 健康检查
├── pm-app-api/               # REST controllers（PM + KB + LLM）
├── pm-app-domain/            # 领域模型（pom 占位，service 实体放 infra 复用 MP）
├── pm-app-infrastructure/    # MyBatis-Plus + Flyway + SDK bean 配置 + Markdown 渲染
└── pm-app-llm/               # LlmProvider 抽象 + NoopLlmProvider(默认) + DeepSeekLlmProvider(可选)
```

### LLM Provider 扩展

`pm-app-llm` 通过 Spring `@Component` 自动发现 `LlmProvider` 实现。新增 provider（如 Doubao / Ollama）：

1. 新建类 `XxxLlmProvider implements LlmProvider`，放 `com.coder.pm.llm.provider` 包，加 `@Component("xxx")`
2. 构造器通过 `@Value` 注入 `pm.llm.xxx.*` 配置项，`isEnabled()` 以 api-key/endpoint 非空为判据
3. 在 `application.yml` 的 `pm.llm` 下加对应配置块
4. 实现 `complete(CompletionRequest)` 返回 `CompletionResponse`——**禁止抛出 RuntimeException**，错误一律返回 `disabled=true` 的响应以保证前端降级

前端通过 `POST /pm-api/v1/llm/requirements/{id}/decompose?provider=xxx` 选择 provider；不带 `provider` 参数则用 `pm.llm.provider.default`。

## 已暴露端点

### System

| Method | Path | 说明 |
|---|---|---|
| GET | `/pm-api/v1/health` | liveness |
| GET | `/pm-api/v1/health/ready` | readiness (DB ping) |
| GET | `/pm-api/actuator/health` | Spring Boot Actuator |
| GET | `/pm-api/v3/api-docs` | OpenAPI JSON |
| GET | `/pm-api/swagger-ui/index.html` | Swagger UI |

### PM CRUD

| Method | Path | Controller |
|---|---|---|
| `POST` `/pm-api/v1/projects` | create | `ProjectController` |
| `GET` `/pm-api/v1/projects` | list | `ProjectController` |
| `GET` `/pm-api/v1/projects/{id}` | detail | `ProjectController` |
| `PATCH` `/pm-api/v1/projects/{id}` | patch | `ProjectController` |
| `DELETE` `/pm-api/v1/projects/{id}` | archive | `ProjectController` |
| `POST` `/pm-api/v1/projects/{id}/requirements` | create | `RequirementController` |
| `GET` `/pm-api/v1/projects/{id}/requirements` | list | `RequirementController` |
| `GET` `/pm-api/v1/requirements/{id}` | detail | `RequirementController` |
| `PATCH` `/pm-api/v1/requirements/{id}` | patch | `RequirementController` |
| `DELETE` `/pm-api/v1/requirements/{id}` | soft-delete | `RequirementController` |
| `POST` `/pm-api/v1/requirements/{id}/stories` | append story | `UserStoryController` |
| `GET` `/pm-api/v1/requirements/{id}/stories` | list | `UserStoryController` |
| `GET` `/pm-api/v1/stories/{id}` | detail | `UserStoryController` |
| `PATCH` `/pm-api/v1/stories/{id}` | patch | `UserStoryController` |
| `POST` `/pm-api/v1/stories/{id}/criteria` | append AC | `AcceptanceCriteriaController` |
| `GET` `/pm-api/v1/stories/{id}/criteria` | list | `AcceptanceCriteriaController` |
| `PATCH` `/pm-api/v1/criteria/{id}` | patch | `AcceptanceCriteriaController` |
| `POST` `/pm-api/v1/criteria/{id}/check` | check/uncheck | `AcceptanceCriteriaController` |

### KB CRUD + 版本管理

| Method | Path | Controller |
|---|---|---|
| `POST` `/pm-api/v1/spaces` | create | `KbSpaceController` |
| `GET` `/pm-api/v1/spaces` | list | `KbSpaceController` |
| `GET` `/pm-api/v1/spaces/{id}` | detail | `KbSpaceController` |
| `PATCH` `/pm-api/v1/spaces/{id}` | patch | `KbSpaceController` |
| `DELETE` `/pm-api/v1/spaces/{id}` | soft-delete | `KbSpaceController` |
| `POST` `/pm-api/v1/spaces/{id}/documents` | create + seed v1 | `KbDocumentController` |
| `GET` `/pm-api/v1/spaces/{id}/documents` | list (?parentId= 树形) | `KbDocumentController` |
| `GET` `/pm-api/v1/documents/{id}` | detail | `KbDocumentController` |
| `PATCH` `/pm-api/v1/documents/{id}` | patch metadata | `KbDocumentController` |
| `POST` `/pm-api/v1/documents/{id}/content` | update content + new version | `KbDocumentController` |
| `DELETE` `/pm-api/v1/documents/{id}` | soft-delete | `KbDocumentController` |
| `GET` `/pm-api/v1/documents/{id}/versions` | list | `KbDocumentVersionController` |
| `GET` `/pm-api/v1/documents/{id}/versions/latest` | latest | `KbDocumentVersionController` |
| `GET` `/pm-api/v1/documents/{id}/versions/{n}` | specific | `KbDocumentVersionController` |
| `POST` `/pm-api/v1/documents/{id}/rollback/{n}` | rollback | `KbDocumentVersionController` |

## 与 low-memory-micro-services 的依赖

| 依赖 | 用途 | SDK 模块 | 当前状态 |
|---|---|---|---|
| audit-service | 操作日志上报 | `audit-service-sdk` | SdkConfig 已创建 `AuditServiceClient` @Bean + `AuditEventReporter`；**6 个 service 共 16 处 audit.report(...) 调用**：ProjectService (create/update/archive) / RequirementService (create/update) / UserStoryService (create/update) / AcceptanceCriteriaService (create/update/check) / KbSpaceService (create/update) / KbDocumentService (create/updateMetadata/updateContent/softDelete) |
| file-service | 知识库附件 | `file-service-sdk` | SdkConfig 已创建 `FileServiceClient` @Bean；调用点下一迭代 |
| notification-service | 评论通知 | `notification-service-sdk` | SdkConfig 已创建 `NotificationServiceClient` @Bean；调用点下一迭代 |
| config-service | 运行时配置 | `config-service-sdk` | SdkConfig 已创建 `ConfigServiceClient` @Bean；调用点下一迭代 |
| iam-service | 用户/角色校验 | `user-service-client-sdk` | 复用 SDK v0.2.0（v0.1.0 未实际调用）|
| api-gateway (3000) | JWT 透传 / 路由 | （SDK 内部统一 baseURL） | config 内 baseURL 已配置 |

> SdkConfig 已就位（`pm-app-infrastructure/config/SdkConfig.java`），4 个 SDK client 都能从 `application.yml` 的 `pm.sdk.*` 注入。**实际 publish：`notification/file/config-service-sdk 0.3.0` 必须先发到 `mvn.kudi.repos` —— 这是当前阻塞 apps/pm 真正上线调用的硬依赖**。

## 表前缀规范

apps/pm 新增两个表前缀，必须在 `scripts/check-db-naming.js` 白名单：

- `pm_*` — 项目管理（`pm_projects` / `pm_requirements` / `pm_user_stories` / `pm_acceptance_criteria`）
- `kb_*` — 知识库（`kb_spaces` / `kb_documents` / `kb_document_versions`）

CI 检查：`node scripts/check-db-naming.js` 已扩展扫描 `apps/**/*.sql`。

## 与 IAM JWT 的关系

**v0.2.0 已与 iam-service JWT 互通**（Phase B 完成）：apps/pm 不再自签 JWT，而是在请求入口验证 iam-service 发行的 RS256 token。

- **JWKS 来源**：通过 `pm.jwt.jwks-url`（默认 `http://localhost:3000/v1/tokens/.well-known/jwks.json`，即 `LOWMEM_GATEWAY_BASE/v1/tokens/.well-known/jwks.json`）拉 RSA 公钥，缓存 `pm.jwt.cache-ttl-minutes` 分钟，缺 `kid` 时强制刷新。
- **issuer / audience / scope**：默认 `superpowers-token-service` / `coder-micro-services` / `access`，与 `services/token-service/src/issuer.rs::issue_user_jwt` 一致。
- **claims 提取**：从 token 抽出 `sub`(UUID) → `userId`(long，取 UUID 前 16 hex 解析)、`user_code`(username)、`tid`(tenantId)、`roles[]`、`permissions[]`、`scope`。`Principal` record 是 Spring `Authentication.principal` 的载体，控制器可 `@AuthenticationPrincipal Principal p` 注入。
- **HS256 dev fallback**：把 `PM_JWT_MODE=hs256` 且 `PM_JWT_HMAC_SECRET=...` 即可启用。生产环境禁止设这俩环境变量。
- **端到端验证**：用 `POST /v1/iam/auth/login` 拿 token → 调 `/pm-api/v1/llm/providers`、`/pm-api/v1/projects` 均 200；伪造 token 全部 403；健康检查 `/pm-api/v1/health` 公开。

实现见 `apps/pm/pm-app-bootstrap/src/main/java/com/coder/pm/web/security/`（`JwksJwtVerifier` / `Hs256JwtVerifier` / `JwtVerifierRouter` / `Principal` / `JwtSecurityConfig`）以及 `JwtAuthFilter`（路由注入用 `@Qualifier(JwtVerifierRouter.BEAN_NAME)`）。

## 部署

docker-compose 已集成（`infrastructure/docker-compose-services.yml` 中的 `pm:` service，端口 8088）。
nginx 代理：在 frontend 的 `nginx.conf` 中加 `/pm-api/` 转发到 `pm:8088/pm-api/`。

## 测试策略

- 默认编译目标：JDK 21（`mvn -Pjava21` 或直接 `mvn`，默认 profile 就是 java21）
- 兼容 JDK 17：`mvn -Pjava17` 输出 17 字节码（仅特殊部署需求，非默认）
- 后续：每个 service 至少 3 个 happy-path + 2 个 error-path 单测
- E2E：`pnpm run test:e2e --filter pm`（下一迭代）

## Dev 演示数据（Phase F2）

跑 `scripts/seed-pm.sh`（默认指向 `http://localhost:8088`，可用 `PM_BASE=...` 覆盖）一次性创建：

- 1 个项目 `DEMO`（id 沿用已存在的，跳过重复创建）
- 2 个 requirements（`用户登录功能` / `KB 文档版本回滚`）
- 2 个 user story + 2 条 AC
- 1 个 KB space `Demo KB` + 1 个 markdown 文档 `Welcome`

幂等：每次跑都按 code/title 查重，不重复创建；多次跑只是补齐缺失项。脚本依赖 `SPRING_PROFILES_ACTIVE=dev`（dev profile 下 NoOpJwtVerifier 启用，无需 Authorization header），同时依赖 `python3`（代替 `jq` 解析响应）。

## 安全上下文工具

`com.coder.pm.infra.security.CurrentActor` 统一读取 Spring `SecurityContextHolder` 内的 `Principal`，提供：

- `actorIdOrNull()` / `actorIdOrZero()`：填 `created_by` 列时用
- `displayName()`：audit report 的 actor 字段
- `principalOrNull()`：需要完整 Principal 时用

域内 6 个 service（Project/Requirement/UserStory/KbSpace/KbDocument + AcceptanceCriteria）已统一通过 `CurrentActor` 拿 actor 信息，不直接接触 `SecurityContextHolder`。新增 entity / service 时也请走 `CurrentActor`，不要内联样板代码。
