# PM Platform — 服务规范

## 必需组件（8-item 服务规范）

- [x] Dockerfile — `apps/pm/Dockerfile`（multi-stage musl-free, temurin JRE 21 runtime）
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
- `PM_JWT_SECRET`: HS256 共享密钥（≥32 字节；生产环境必读 ENV）
- `PM_JWT_EXPIRATION_MINUTES`: token TTL，默认 60 分钟
- `PM_LLM_PROVIDER`: LLM provider 名，默认 noop（noop / deepseek）
- `DEEPSEEK_API_KEY` / `DEEPSEEK_BASE_URL` / `DEEPSEEK_MODEL`: DeepSeek (OpenAI-compatible) 接入配置；api-key 非空时 deepseek provider 自动启用
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

v0.1.0 **不**与 IAM JWT 互通：apps/pm 自己签发 HS256 JWT（`JwtConfig` + `JwtAuthFilter`）；当前未实现 `/pm-api/v1/auth/login`，`SecurityContext` 在控的请求由网关或开发模式 dev-bypass 注入。
**未来计划（v0.3+）**：切换为 RS256 + 共享 JWKS，让用户可以 IAM 登录后无缝进入 PM 模块。

## 部署

docker-compose 已集成（`infrastructure/docker-compose-services.yml` 中的 `pm:` service，端口 8088）。
nginx 代理：在 frontend 的 `nginx.conf` 中加 `/pm-api/` 转发到 `pm:8088/pm-api/`。

## 测试策略

- 当前阶段：mvn compile + Spring Boot 启动验证（环境无 JDK 17/Maven 未能本地跑 verify）
- 后续：每个 service 至少 3 个 happy-path + 2 个 error-path 单测
- E2E：`pnpm run test:e2e --filter pm`（下一迭代）
