# PM Platform — 禅道替代研发管理工具

> 日期：2026-07-11
> 状态：**设计阶段（Phase 1 / 落地执行见后续 PR）**
> 范围：apps/pm/（Spring Boot 3.x 业务应用） + sdk/java/（多语言 SDK 补全） + frontend/（内嵌模块）

---

## 1. 动机与定位

公司当前 `coder-micro-services` 平台围绕 IAM / 通知 / 审计 / 文件 / 配置五大基础设施服务提供 RBAC、JWT、WebSocket、上传下载、动态配置。但**业务应用层缺位**——没有需求管理、文档协作、知识沉淀等"研发管理工具链"。

本设计填补这一层：

- **目标用户**：内部研发团队（小团队到中型，5–50 人）
- **MVP 范围**：项目管理（项目/需求/用户故事/验收标准） + 知识库（空间/文档/版本/Markdown） + LLM 接口预留（trait/interface 抽象、不接真实 LLM）
- **不复刻禅道全量**：不做产品/迭代/任务/缺陷/用例/统计/Gantt/集成（属于"中等范围"+"完整复刻"两个延后级别）

---

## 2. 架构总览

```
                         ┌──────────────────────────────────┐
   Browser ────Nginx────►│ vue-pure-admin (frontend/)       │
   (vue3)        │       │  ├─ /pm/projects       新增路由 │
                  │       │  ├─ /pm/requirements   新增路由 │
                  │       │  ├─ /pm/kb             新增路由 │
                  │       │  └─ http client        新增   │
                  │       └──────┬──────────────┬────────┘
                  │              │ /api/*       │ /pm-api/*
                  │              ▼              ▼
                  │   ┌────────────────────┐  ┌────────────────────┐
                  │   │ api-gateway (Rust) │  │ apps/pm (Spring    │
                  │   │   :3000 JWT验签    │  │   Boot 3.x)        │
                  │   └─┬────┬────┬───┬───┘  │   :8088            │
                  │     │    │    │   │       │ ├─ JWT 自签（HS256）│
                  │     ▼    ▼    ▼   ▼       │ ├─ 业务库 superpowers│
                  │   iam  token audit file   │ └─ /pm/* 路由     │
                  │   svc  svc   svc  svc     └──┬────────┬──────┘
                  │                             │ SDK    │ SDK
                  │                             ▼        ▼
                  │                       iam/file/audit SDK
                  │                       (调 api-gateway)
                  │                             │
                  └─────────────────────────────┘
                       共享 MySQL `superpowers`
                       ├─ iam_*/audit_*/file_*/... (现有 Rust)
                       └─ pm_*/kb_* (新增, 本设计)
```

**关键决策**：

1. **业务应用是独立 Java 服务**，不并入 api-gateway。原因：(a) api-gateway 是 Rust 写的、不希望为加一种语言引入 docker-in-docker 或 sidecar；(b) Spring Security + 自签 JWT 足够 MVP 鉴权；(c) apps/pm 通过 SDK 调 Rust 服务，鉴权仍然走 api-gateway 路径。
2. **apps/pm 自己签发 JWT**（HS256、对称密钥、共享给前端）。原因：MVP 简化、不引入 OAuth2 dance；后期可改成调 iam-service 签发或对接 OIDC。
3. **apps/pm 直接连 MySQL** `superpowers` 库、复用 db 实例、严格遵循 `contracts/db-conventions.md` 表前缀规则（新增 `pm_` 和 `kb_` 两个前缀）。
4. **SDK 增量补全**：notification-service-sdk、file-service-sdk、config-service-sdk 三个新模块（沿用现有 `audit-service-sdk` 模式：单一 facade 类 + Jackson DTO + okhttp 调用）。

---

## 3. 数据模型（ER 图）

```
┌─────────────────────────────────────┐
│ pm_projects (项目)                   │
│  id BIGINT PK                       │
│  code VARCHAR(32) UK                │
│  name VARCHAR(128)                  │
│  description TEXT                   │
│  status ENUM('active','archived')   │
│  owner_id BIGINT                    │───► iam_users.id (跨服务, 不FK)
│  created_at, updated_at             │
└────────┬──────────────┬─────────────┘
         │              │
         ▼              ▼
┌────────────────────┐ ┌──────────────────────────────┐
│ pm_requirements    │ │ kb_spaces (知识库空间)         │
│  id BIGINT PK      │ │  id BIGINT PK                 │
│  project_id FK     │ │  project_id FK NULL (全局空间) │
│  parent_id FK NULL │ │  name VARCHAR(128)            │
│  title VARCHAR(256)│ │  description TEXT             │
│  description TEXT  │ │  visibility ENUM              │
│  type ENUM         │ │   ('private','project','open')│
│   ('feature',      │ │  created_at, updated_at       │
│    'improvement',  │ └────────┬─────────────────────┘
│    'bug','task')   │          │
│  priority ENUM     │          ▼
│   ('p0','p1','p2', │ ┌──────────────────────────────┐
│    'p3')           │ │ kb_documents (文档)            │
│  status ENUM       │ │  id BIGINT PK                  │
│   ('draft','open', │ │  space_id FK                   │
│    'in_progress',  │ │  parent_id FK NULL (目录树)    │
│    'resolved',     │ │  title VARCHAR(256)            │
│    'closed',       │ │  current_version_id FK NULL    │
│    'rejected')     │ │  doc_type ENUM                 │
│  estimate_hours INT│ │   ('markdown','api_spec',     │
│  assignee_id BIGINT│ │    'design_doc','meeting')     │
│  created_at, up_at │ │  created_by, updated_at        │
└────────┬───────────┘ └────────┬─────────────────────┘
         │                      │
         ▼                      ▼
┌──────────────────────┐  ┌───────────────────────────┐
│ pm_user_stories      │  │ kb_document_versions       │
│  id BIGINT PK        │  │  id BIGINT PK              │
│  requirement_id FK   │  │  document_id FK            │
│  as_a VARCHAR(128)   │  │  version_no INT            │
│  i_want TEXT         │  │  title VARCHAR(256)        │
│  so_that TEXT        │  │  content_md LONGTEXT       │
│  story_points TINYINT│  │  content_html LONGTEXT     │
│  status ENUM         │  │   (rendered, 缓存)         │
│   ('pending',        │  │  editor_id BIGINT          │
│    'in_progress',    │  │  edit_summary VARCHAR(255) │
│    'done','rejected')│  │  created_at                │
│  sort_order INT      │  └───────────────────────────┘
│  created_at, up_at   │           │ UK (document_id, version_no)
└────────┬─────────────┘           │ ◄── kb_documents.current_version_id
         │
         ▼
┌──────────────────────────────────┐
│ pm_acceptance_criteria           │
│  id BIGINT PK                    │
│  user_story_id FK                │
│  given TEXT                      │
│  when TEXT                       │
│  then TEXT                       │
│  checked TINYINT (0/1)           │
│  sort_order INT                  │
│  created_at, updated_at          │
└──────────────────────────────────┘
```

**说明**：

- **跨服务用户引用**（`owner_id` / `assignee_id` / `created_by`）：存 `BIGINT` 但**不设物理外键**。理由：iam-service 是独立服务，表物理隔离；apps/pm 通过 SDK 在写入前做存在性校验。
- **租户隔离**：MVP 不引入多租户（与 iam-service 不同，禅道替代工具通常是单租户团队工具）。
- **软删除**：所有业务表加 `deleted_at` 列、查询统一过滤。
- **审计**：操作日志通过 audit-service-sdk 上报（含 actor、target、action、payload）。

---

## 4. 模块结构

### 4.1 `apps/pm/` Spring Boot 3.x 多模块 Maven

```
apps/pm/
├── pom.xml                         # parent: spring-boot-dependencies 3.3.x
├── pm-app-bootstrap/               # @SpringBootApplication 入口
│   ├── pom.xml
│   └── src/main/java/com/coder/pm/
│       ├── PmApplication.java
│       ├── config/
│       │   ├── SecurityConfig.java
│       │   ├── JwtConfig.java
│       │   ├── OpenApiConfig.java  # springdoc-openapi
│       │   └── CorsConfig.java
│       └── web/
│           ├── exception/GlobalExceptionHandler.java
│           └── filter/JwtAuthFilter.java
│
├── pm-app-api/                     # REST 接口层（Controller + DTO）
│   ├── pom.xml
│   └── src/main/java/com/coder/pm/api/
│       ├── project/{ProjectController,ProjectDto}
│       ├── requirement/{RequirementController,RequirementDto}
│       ├── story/{UserStoryController,StoryDto}
│       ├── acceptance/{AcceptanceCriteriaController,Dto}
│       ├── kb/{SpaceController,DocumentController,VersionController,Dto}
│       └── llm/{RequirementAnalysisController,AnalysisDto}
│
├── pm-app-domain/                  # 领域模型 + 业务规则
│   ├── pom.xml
│   └── src/main/java/com/coder/pm/domain/
│       ├── project/{Project,ProjectService,ProjectRepository}
│       ├── requirement/{Requirement,RequirementService,Repository,
│       │                RequirementStatus,RequirementPriority}
│       ├── story/{UserStory,UserStoryService,Repository,
│       │          StoryStatus}
│       ├── acceptance/{AcceptanceCriteria,Service,Repository}
│       ├── kb/{Space,Document,DocumentVersion,Services,Repositories}
│       └── llm/{LlmProvider,RequirementAnalyzer,PromptTemplate}
│
├── pm-app-infrastructure/          # MyBatis-Plus + Flyway + 集成
│   ├── pom.xml
│   ├── src/main/java/com/coder/pm/infra/
│   │   ├── persistence/
│   │   │   ├── entity/             # @TableName 对应 pm_*/kb_* 表
│   │   │   ├── mapper/             # MyBatis-Plus BaseMapper
│   │   │   └── repository/         # 仓储实现
│   │   ├── sdk/                    # 调 low-memory-micro-services SDK
│   │   │   ├── AuditEventReporter.java
│   │   │   ├── FileAttachmentService.java
│   │   │   └── IamUserValidator.java   # 写时校验用户存在
│   │   └── markdown/MarkdownRenderer.java  # flexmark-java
│   └── src/main/resources/db/migration/
│       ├── V1__init_pm_tables.sql
│       └── V1__init_kb_tables.sql
│
└── pm-app-llm/                     # LLM 抽象层（不接真实 LLM）
    ├── pom.xml
    └── src/main/java/com/coder/pm/llm/
        ├── LlmProvider.java        # interface: CompletionRequest/Response
        ├── LlmProviderRegistry.java
        ├── impl/NoopLlmProvider.java  # 返回固定响应 + 标记"未启用"
        ├── prompt/RequirementDecomposePrompt.java
        └── model/{ChatMessage,CompletionRequest,CompletionResponse}
```

### 4.2 `sdk/java/` 增量补全

| 新模块 | 用途 | 关键类 |
|---|---|---|
| `notification-service-sdk` | 上报通知 | `NotificationClient.publish(String topic, Object payload)` |
| `file-service-sdk` | 文件元数据 + 预签名上传 | `FileClient.upload(MultipartFile)` / `getPresignedUrl(fileId)` |
| `config-service-sdk` | 命名空间配置项读写 | `ConfigClient.get(namespace, key)` / `put(...)` / `watch(...)` |

命名沿用 `audit-service-sdk` 模式：每个 SDK 模块是单一 facade 类 + DTO + okhttp，依赖 `kudi-coder-sdk-common` 共享 TokenCache / HttpClient / SdkException。

### 4.3 `frontend/` 内嵌

```
frontend/src/
├── api/pm.ts                       # http.request('/pm-api/v1/...')
├── api/kb.ts
├── api/llm.ts
├── store/modules/pm.ts             # Pinia
├── views/pm/
│   ├── project/
│   │   ├── list.vue
│   │   ├── detail.vue
│   │   └── components/{Form,Table}
│   ├── requirement/
│   │   ├── list.vue
│   │   ├── detail.vue              # 含 AI 拆解按钮
│   │   └── components/{UserStoryList,AcceptanceList}
│   └── kb/
│       ├── space-list.vue
│       ├── document-tree.vue
│       ├── document-edit.vue       # Monaco Editor 或 md-editor-v3
│       └── document-view.vue       # 渲染 Markdown
├── router/modules/pm.ts            # /pm/* 路由
└── layouts/components/Sidebar/    # 侧边栏加 PM 模块入口
```

---

## 5. API 设计（REST v1）

所有路径前缀 `/pm-api/v1`（nginx 路由 apps/pm）。

| 模块 | Method + Path | 说明 |
|---|---|---|
| **认证** | `POST /pm-api/v1/auth/login` | username + password → JWT |
| | `POST /pm-api/v1/auth/refresh` | refresh JWT |
| | `GET  /pm-api/v1/auth/me` | 当前用户信息 |
| **项目** | `POST   /pm-api/v1/projects` | 创建项目 |
| | `GET    /pm-api/v1/projects?page=&size=` | 分页列表 |
| | `GET    /pm-api/v1/projects/{id}` | 详情（含需求数/文档数） |
| | `PATCH  /pm-api/v1/projects/{id}` | 更新 |
| | `DELETE /pm-api/v1/projects/{id}` | 软删 |
| **需求** | `POST   /pm-api/v1/projects/{id}/requirements` | 创建需求 |
| | `GET    /pm-api/v1/projects/{id}/requirements?status=&type=` | 列表 |
| | `GET    /pm-api/v1/requirements/{id}` | 详情（含故事/AC 树） |
| | `PATCH  /pm-api/v1/requirements/{id}` | 更新（状态流转校验） |
| | `DELETE /pm-api/v1/requirements/{id}` | 软删 |
| **故事** | `POST   /pm-api/v1/requirements/{id}/stories` | 追加故事 |
| | `PATCH  /pm-api/v1/stories/{id}` | 更新 |
| | `POST   /pm-api/v1/stories/{id}/sort` | 拖拽排序 |
| **验收标准** | `POST   /pm-api/v1/stories/{id}/criteria` | 追加 AC |
| | `PATCH  /pm-api/v1/criteria/{id}` | 更新 |
| | `POST   /pm-api/v1/criteria/{id}/check` | 勾选/取消 |
| **知识库空间** | `POST/GET/PATCH/DELETE /pm-api/v1/spaces` | 空间 CRUD |
| **文档** | `POST   /pm-api/v1/spaces/{id}/documents` | 创建文档 |
| | `GET    /pm-api/v1/spaces/{id}/documents` | 树形列表 |
| | `GET    /pm-api/v1/documents/{id}` | 详情（含版本号） |
| | `PATCH  /pm-api/v1/documents/{id}` | 更新（写新版本） |
| | `DELETE /pm-api/v1/documents/{id}` | 软删 |
| | `GET    /pm-api/v1/documents/{id}/versions` | 版本列表 |
| | `GET    /pm-api/v1/documents/{id}/versions/{n}` | 读取指定版本 |
| | `POST   /pm-api/v1/documents/{id}/rollback/{n}` | 回滚到指定版本 |
| **LLM 辅助** | `POST   /pm-api/v1/llm/requirements/{id}/decompose` | 拆解需求 → 用户故事 + AC |
| | `POST   /pm-api/v1/llm/requirements/{id}/refine` | 改写需求描述 |
| | `GET    /pm-api/v1/llm/providers` | 当前可用 provider 列表 |
| **健康** | `GET    /pm-api/v1/health` | liveness（不依赖 DB） |
| | `GET    /pm-api/v1/health/ready` | readiness（DB ping + SDK ping） |

**统一响应**：

```json
// 成功
{ "code": 0, "message": "ok", "data": { ... } }
// 失败
{ "code": 40001, "message": "validation failed", "data": null, "traceId": "..." }
// 分页
{ "code": 0, "data": { "items": [...], "page": 1, "size": 20, "total": 137 } }
```

错误码占位 `1xxxx`（PM）、`2xxxx`（KB）、`9xxxx`（通用）。**不引入新错误码到 `contracts/error-codes.yaml`**——Spring Boot 内独立错误码（避免与 Rust 服务的 IAM/AUD/NTF 错误码冲突；apps/pm 是独立业务上下文）。

---

## 6. LLM 接口（Java）抽象设计

```java
// pm-app-domain LlmProvider.java
public interface LlmProvider {
    String name();   // "noop" | "openai" | "doubao" | "ollama"
    CompletionResponse complete(CompletionRequest req);
    boolean isEnabled();   // false 时返回 Noop 响应
}

// pm-app-llm LlmProviderRegistry.java
@Component
public class LlmProviderRegistry {
    private final Map<String, LlmProvider> providers;
    public CompletionResponse dispatch(String providerName, CompletionRequest req) {
        var p = providers.getOrDefault(providerName, providers.get("noop"));
        if (!p.isEnabled()) {
            return CompletionResponse.disabled("LLM not configured");
        }
        return p.complete(req);
    }
}

// pm-app-llm impl NoopLlmProvider.java（MVP 默认）
public class NoopLlmProvider implements LlmProvider {
    public String name() { return "noop"; }
    public boolean isEnabled() { return false; }
    public CompletionResponse complete(CompletionRequest req) {
        return CompletionResponse.builder()
            .provider("noop")
            .content("⚠️ LLM 未启用：请在 application.yml 配置 pm.llm.provider=openai 等")
            .finishReason("disabled")
            .build();
    }
}

// pm-app-llm prompt RequirementDecomposePrompt.java
public class RequirementDecomposePrompt {
    public static final String SYSTEM = """
        你是一个高级软件需求分析师。请将用户输入的需求拆解为：
        1. 用户故事（As a... I want... so that...）
        2. 验收标准（Given/When/Then）
        输出严格 JSON 格式，包含 stories[] 和 criteria_per_story[]。
        """;
    public static String buildUserPrompt(String title, String description) { ... }
}
```

**关键**：MVP **不接**真实 LLM。`NoopLlmProvider` 是唯一实现。`LlmProviderRegistry` 留好扩展点，未来直接加 `OpenAiLlmProvider` 等实现、配置切换即可。AI 拆解按钮在前端禁用 + 提示文案。

---

## 7. SDK 调用关系

```
apps/pm
  ├─ IamUserValidator
  │     └─ user-service-user-sdk → GET /v1/iam/users/{id}
  │     (写入前校验 owner_id / assignee_id 存在)
  │
  ├─ AuditEventReporter
  │     └─ audit-service-sdk → POST /v1/audit/events
  │     (操作日志：创建需求/更新文档/...)
  │
  ├─ FileAttachmentService
  │     └─ file-service-sdk → POST /v1/file/files/presign
  │     (文档附件预签名上传)
  │
  └─ ConfigClient
        └─ config-service-sdk → GET /v1/config/items/{ns}/{key}
        (运行时配置：LLM provider、Markdown 渲染选项)
```

SDK 内部通过 `KudiHttpClient`（已有）+ JWT 透传：apps/pm 拿到自家 JWT 后，直接以 `Authorization: Bearer <jwt>` 转发给 api-gateway；api-gateway 验签后按路径路由到目标 Rust 服务。**注意**：api-gateway 当前只对 Rust 业务服务鉴签通过——apps/pm 自己签发的 JWT 用相同算法（HS256）和共享密钥，理论上能被 api-gateway 识别（如有需要可后续把密钥换成 RSA）。

MVP 简化：**apps/pm 调用 SDK 时，先拿 IAM token**（用 `user-service-client-sdk` 的 client_credentials 流程，作为服务身份），再调目标服务。apps/pm 自己的用户登录走自家 JWT（不与 IAM 互通）。

---

## 8. 数据库迁移（Flyway V1）

```sql
-- apps/pm/src/main/resources/db/migration/V1__init_pm_tables.sql
CREATE TABLE pm_projects (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  code VARCHAR(32) NOT NULL,
  name VARCHAR(128) NOT NULL,
  description TEXT,
  status ENUM('active','archived') NOT NULL DEFAULT 'active',
  owner_id BIGINT NOT NULL,
  deleted_at DATETIME,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  UNIQUE KEY uk_pm_projects_code (code),
  KEY idx_pm_projects_owner (owner_id),
  KEY idx_pm_projects_status (status, deleted_at)
);

CREATE TABLE pm_requirements (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  project_id BIGINT NOT NULL,
  parent_id BIGINT,
  title VARCHAR(256) NOT NULL,
  description TEXT,
  type ENUM('feature','improvement','bug','task') NOT NULL,
  priority ENUM('p0','p1','p2','p3') NOT NULL DEFAULT 'p2',
  status ENUM('draft','open','in_progress','resolved','closed','rejected') NOT NULL DEFAULT 'draft',
  estimate_hours INT,
  assignee_id BIGINT,
  deleted_at DATETIME,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  KEY idx_pm_req_project (project_id, status, deleted_at),
  KEY idx_pm_req_assignee (assignee_id, status)
);

CREATE TABLE pm_user_stories (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  requirement_id BIGINT NOT NULL,
  as_a VARCHAR(128),
  i_want TEXT,
  so_that TEXT,
  story_points TINYINT,
  status ENUM('pending','in_progress','done','rejected') NOT NULL DEFAULT 'pending',
  sort_order INT NOT NULL DEFAULT 0,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  KEY idx_pm_stories_req (requirement_id, sort_order)
);

CREATE TABLE pm_acceptance_criteria (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  user_story_id BIGINT NOT NULL,
  given_text TEXT,
  when_text TEXT,
  then_text TEXT,
  checked TINYINT(1) NOT NULL DEFAULT 0,
  sort_order INT NOT NULL DEFAULT 0,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  KEY idx_pm_ac_story (user_story_id, sort_order)
);
```

`V1__init_kb_tables.sql` 类似：`kb_spaces` / `kb_documents` / `kb_document_versions`。

**表前缀规则**：`pm_` 和 `kb_` 是新增前缀，需要在 `scripts/check-db-naming.js` 的白名单加：
```js
const TABLE_PREFIX_WHITELIST = ['iam_','audit_','token_','ntf_','cfg_','file_','prue_','pm_','kb_'];
```

---

## 9. 部署与运行

```yaml
# infrastructure/docker-compose.yml 加 services.pm
  pm:
    build:
      context: ..
      dockerfile: apps/pm/pm-app-bootstrap/Dockerfile
    ports:
      - "8088:8088"
    environment:
      SERVER_PORT: 8088
      SPRING_PROFILES_ACTIVE: dev
      SPRING_DATASOURCE_URL: jdbc:mysql://mysql:3306/superpowers?useSSL=false&serverTimezone=UTC
      SPRING_DATASOURCE_USERNAME: root
      SPRING_DATASOURCE_PASSWORD: rootpass
      PM_JWT_SECRET: dev-secret-change-me
      PM_LLM_PROVIDER: noop
      LOWMEM_GATEWAY_BASE: http://api-gateway:3000
    depends_on:
      mysql: { condition: service_healthy }
      api-gateway: { condition: service_started }
```

```nginx
# frontend/nginx.conf 加 /pm-api 路由
location /pm-api/ {
    proxy_pass http://pm:8088/pm-api/;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

---

## 10. 验证清单

### 10.1 SDK

- [ ] `node scripts/build-java-sdk-docker.js compile --profile java17` 通过
- [ ] `node scripts/build-java-sdk-docker.js test` 全绿
- [ ] 三个新 SDK 模块有至少 1 个 happy-path 单测（用 `mockwebserver`）
- [ ] `sdk/java/README.md` 更新模块表格
- [ ] 0.2.0 → 0.3.0 版本号 bump（新增 module）

### 10.2 apps/pm

- [ ] `cd apps/pm && mvn -Pjava17 clean verify` 通过（本地 mvn 17）
- [ ] `docker compose -f infrastructure/docker-compose.yml up -d pm` 起得来
- [ ] `curl http://localhost:8088/pm-api/v1/health` → 200
- [ ] Flyway 迁移成功（mysql 容器里 `SHOW TABLES LIKE 'pm_%'` 有结果）
- [ ] swagger-ui 可访问 `/pm-api/swagger-ui`
- [ ] 端到端：登录 → 创建项目 → 创建需求 → 拆故事 → 拆 AC → 创建知识库空间 → 写文档

### 10.3 frontend

- [ ] `pnpm --filter @coder/frontend run build` 通过
- [ ] `pnpm --filter @coder/frontend run test:unit` 全绿
- [ ] `/pm/projects`、`/pm/requirements/{id}`、`/pm/kb/spaces/{id}` 三个页面渲染正常
- [ ] 表单提交、Markdown 预览、版本切换功能可用

### 10.4 集成

- [ ] docker compose up 全部起得来
- [ ] 浏览器走完 happy-path：登录 IAM → 进入 PM 模块 → 创建项目 → 操作需求 → 写入文档
- [ ] audit-service 上报有 apps/pm 的事件记录

---

## 11. NOT in scope（明确延后）

| 项 | 原因 |
|---|---|
| 任务 / Bug / 用例管理 | MVP 不做；待 PM/KB 跑通后再加 |
| 迭代 (Sprint) | 同上 |
| 统计 / 报表 / Gantt | 同上 |
| 评论 / @提醒 / WebSocket 实时协作 | 知识库 MVP 只做单人写；后期考虑通知服务集成 |
| 多租户 | 单团队工具，不需要 |
| 接入真实 LLM（火山方舟 / OpenAI / Ollama） | 用户明确说"暂不集成，留接口" |
| OAuth2 / OIDC 与 iam-service 互签 JWT | MVP 自签 JWT；后期换 RS256 + JWK |
| 全文检索（ES / Meilisearch） | 文档搜索先用 SQL LIKE；量大再上 ES |
| CI gate 整合（service-standard / contracts-gen） | apps/pm 不是 Rust 服务；新增 `scripts/check-java-app-standard.js` 单独门禁 |
| Docker 镜像瘦身（jlink / distroless） | MVP 用 `eclipse-temurin:21-jre-alpine` 足够 |

---

## 12. 风险与对策

| 风险 | 影响 | 对策 |
|---|---|---|
| apps/pm 自签 JWT 与 iam-service 不互通 | 前端登录后切不到 PM 模块 | MVP 阶段前后端用独立登录（PM 模块单独登录页）；后期改 RS256 + 共享 JWKS |
| 表前缀 `pm_` / `kb_` 没加入 `check-db-naming.js` 白名单 | CI 漂移失败 | 本 PR 一并更新脚本 |
| SDK 用 okhttp 4.x + Spring Boot 3.x 自带 okhttp 4.x 可能冲突 | ClassLoader 冲突 | SDK 模块用 `provided` scope 暴露依赖，业务侧由 Spring Boot 容器统一管理 |
| Markdown 渲染 XSS | 知识库文档渲染被注入 | flexmark-java 默认配置 + 白名单 sanitizer；前端 Markdown 组件用 DOMPurify |
| MyBatis-Plus 逻辑删除 + 自定义 SQL 漏过滤 `deleted_at` | 软删数据泄露 | 全局 `@SqlParser(filter = true)` + 强制 SQL hint + 测试覆盖 |
| apps/pm 与 Rust 服务版本号不一致（破坏性变更） | SDK 升级失败 | SDK 0.3.0 起，主版本对齐后端；Java 应用锁定 SDK 版本范围 `[0.3.0, 0.4.0)` |

---

## 13. 后续 PR 拆分（一个 commit 一个 PR）

1. **PR-1 / SDK 补全**：notification-service-sdk + file-service-sdk + config-service-sdk + sdk/java/pom.xml + README + 0.2.0 → 0.3.0
2. **PR-2 / apps/pm 骨架**：parent pom + 5 模块 pom + Spring Boot 启动类 + Flyway V1 + 健康检查 + Dockerfile
3. **PR-3 / PM 业务**：项目/需求/用户故事/AC 的 domain + infrastructure + api
4. **PR-4 / KB 业务**：空间/文档/版本的 domain + infrastructure + api + Markdown 渲染
5. **PR-5 / LLM 接口**：LlmProvider 接口 + NoopLlmProvider + 拆解 API + 前端按钮（disabled + 提示）
6. **PR-6 / 前端集成**：vue-pure-admin 路由 + 视图 + API 客户端 + Pinia + 侧边栏
7. **PR-7 / 部署**：docker-compose.yml + nginx.conf + docker builder 镜像更新
8. **PR-8 / 验证 + 文档**：E2E + 单测覆盖率报告 + AGENTS.md + README + 路线图追加