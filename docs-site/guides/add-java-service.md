# 如何新增一个 Java 业务微服务

> 端到端实战手册：从「我想加个 Java 业务服务（如合同管理）」到「CI 门禁全绿、经网关可达」。
> 配套 skill：`.agents/skills/coder-new-java-service/`（AI 脚手架细节与完整文件模板）。本文面向**人 + AI 协作**，给流程、给决策、给坑，不重复逐行模板。
>
> 姊妹篇：
> - 新增 **Rust** 服务 → [新增服务（Rust）](./add-new-service)
> - **外部 Java 应用**作为客户端调本平台 API → [Java 集成](./java-integration)
>
> 本文覆盖的是第三种场景：**在本仓库 `services/` 内新建一个 Spring Boot 业务微服务**，与现有 Rust 服务并列、经同一个 api-gateway 对外。

全文以「合同管理」为贯穿示例，服务名 `contract-service`。换服务时整体替换命名占位符即可（见 §1）。

---

## 0. 先想清楚：该新建 Java 服务吗？

平台里绝大多数服务是 Rust + Axum。新建 **Java** 服务前，先确认确实需要：

- 团队技术栈是 Java / Spring 生态，或要复用现成的 Java 业务库（流程引擎、报表、特定 SDK 等）；
- 是一个**新的业务限界上下文**，有独立部署节奏，而不是给现有服务加个路由。

如果只是加个表/接口，优先在现有服务里加路由（Rust 用 `coder-add-route`，Java 用 `coder-add-java-route`）。**服务数量是稀缺资源**：每加一个都要喂 8 项规范、CI 门禁、网关路由登记、表前缀登记、文档同步。

> Java 服务与 Rust 服务的**最大差异**：Spring Boot 自带 `context-path`（Servlet context），而 Rust 服务的路由是裸路径。这直接决定了网关 `downstream` 怎么写、健康检查 URL 怎么配——是 Java 服务最容易踩的坑，本文 §5 专门讲。

---

## 1. 取名字（一次性敲定，所有下游配置都用这一组）

以合同管理为例：

| 用途 | 命名 | 合同示例 |
|------|------|----------|
| 服务目录 / 容器 / 镜像名 | `kebab-case`，`xxx-service` | `contract-service` |
| Maven 模块短名（去掉 `-service`） | `{short}` | `contract` |
| Java 包名段（无连字符） | 小写 | `com.coder.contract` |
| PascalCase 短名 | `{Prefix}` | `Contract` |
| **Servlet context-path** | `/{short}-api` | `/contract-api` |
| **网关对外路径前缀** | `/v1/{short}/...`（单数） | `/v1/contract/...` |
| 业务表前缀（共享 MySQL） | `{short}_` | `contract_` |
| 权限码前缀 | `<SERVICE>_<RESOURCE>_<ACTION>` | `CONTRACT_CONTRACT_CREATE` |
| 错误码前缀 | `{SERVICE}_<类别>_<号>` | `CONTRACT_40401` |
| 宿主映射端口 | 8088 起，逐个递增 | `8088:8080` |
| **容器内监听端口** | **固定 `8080`**（Spring Boot 默认） | `8080` |

**强约束**：

- **容器内端口固定 `8080`**。宿主侧端口（`:8088`）只用于宿主机 `curl` 联调，网关走容器内网络、不看宿主端口。端口占用见 [`user-manual/11-reference`](../user-manual/11-reference.md) 端口表（8080–8087 已被 Rust 服务占用，Java 从 8088 起映射）。
- 表前缀必须先登记（`scripts/check-db-naming.js` 的前缀数组 **和** `contracts/db-conventions.md` 映射表），CI 才放过；同一个服务只能有一个表前缀。
- 权限码 / 错误码先在 `contracts/permissions.yaml`、`contracts/error-codes.yaml` 加，再 `node scripts/contracts-gen.js` 重新生成。

---

## 2. 前期准备清单（动手前必须有答案）

| 项 | 合同示例答案 |
|---|---|
| 服务名 | `contract-service` |
| 一句话业务定位 | 合同的创建、审批、归档与查询 |
| 技术栈 | Spring Boot 3.3.x + JDK 21 + MyBatis-Plus + Flyway（MySQL） |
| 数据库 | 共享 MySQL `superpowers` 库，表前缀 `contract_` |
| context-path | `/contract-api` |
| 端口 | 容器内 `8080`，宿主 `8088` |
| 是否调用其它平台服务 | 例：写审计（audit-sdk）、传合同附件（file-sdk）、发通知（notification-sdk）——决定放开哪些 SDK 依赖 |
| 初始路由 | 例：合同 CRUD `/v1/contract/**`（后续用 `coder-add-java-route` 补） |

---

## 3. 脚手架（让 AI 干活）

直接调 skill：

```
触发词：「新增一个 Java 合同服务 contract-service」/「scaffold a java service」/「coder-new-java-service」
```

skill 会生成满足 **8 项服务基线**（ADR-002）的多模块 Maven 工程，落在 `services/contract-service/`：

```
services/contract-service/
├── pom.xml                       # 父 POM（packaging=pom），聚合 4 模块
├── Dockerfile                    # 3 阶段：构建本地 Java SDK → 构建服务 → JRE alpine 运行时
├── AGENTS.md                     # 服务约定（从 services/agents-template.md 改）
├── README.md                     # Quick-start + 端点表
├── config/application.yml        # 外置配置（compose 挂载目标）
├── contract-app-bootstrap/       # 入口 + Security + 健康检查 + 全局异常
│   └── src/main/java/com/coder/contract/
│       ├── ContractApplication.java     # @SpringBootApplication + @MapperScan
│       ├── config/OpenApiConfig.java
│       ├── config/SecurityConfig.java   # 过滤链 + CORS
│       └── web/
│           ├── HealthController.java    # /v1/health + /v1/health/ready
│           ├── JwtAuthFilter.java       # 解码 JWT（不验签，信任网关）
│           └── exception/GlobalExceptionHandler.java
├── contract-app-api/             # ★ Controller + DTO 放这里
├── contract-app-domain/          # 领域异常（NotFoundException / ConflictException）
├── contract-app-infrastructure/  # MyBatis-Plus + Flyway + SDK bean
│   └── src/main/resources/db/migration/V1__init.sql
└── tests/                        # 集成测试占位
```

几个**必须对**的点（skill 模板已处理，人工 review 时重点看）：

- `ContractApplication` 带 `@MapperScan("com.coder.contract.infra.persistence.mapper")`，否则 MyBatis mapper 扫不到。
- Controller 放 **`contract-app-api`** 模块，不要塞进 bootstrap。
- `application.yml` 里 `server.servlet.context-path: /contract-api`，且 `server.port: 8080`。
- Dockerfile 构建上下文是**仓库根**（compose 里 `context: ..`），这样 Stage 1 才能 `COPY sdk/java/` 构建本地 SDK。

### 认证：信任网关，不验签

`JwtAuthFilter` **只解码 JWT payload、不验签名**——api-gateway 已用 token-service 的 JWKS 验过 RS256 签名、查过 Redis 黑名单、核对过 perm_epoch，然后把 `Authorization: Bearer <jwt>` 原样透传。过滤器把 `tid / roles / permissions / sub` 解出来装进 `Principal`，`permissions` 映射成 Spring authorities，于是 Controller 上可以直接：

```java
@PreAuthorize("hasAuthority('CONTRACT_CONTRACT_READ')")
@GetMapping("/{id}")
public ContractDto get(@PathVariable Long id) { ... }
```

> **用户引用列必须 `VARCHAR(64)`**：平台 user id 是随机字符串，不是自增整数。`created_by` / `owner_id` / `updated_by` 等列**严禁 `BIGINT`**（见 `contracts/db-conventions.md` §「用户引用字段类型约定」）。

---

## 4. 数据库迁移

Java 服务用 **Flyway**（不像 Rust 用 sqlx 迁移），SQL 放在 `contract-app-infrastructure/src/main/resources/db/migration/V1__init.sql`。表名全部带 `contract_` 前缀：

```sql
-- V1__init.sql
SET NAMES utf8mb4;

CREATE TABLE contract_contracts (
    id          BIGINT NOT NULL AUTO_INCREMENT,
    title       VARCHAR(256) NOT NULL,
    status      VARCHAR(32)  NOT NULL DEFAULT 'draft',
    owner_id    VARCHAR(64)  NOT NULL,        -- ★ 用户引用：VARCHAR(64)，不是 BIGINT
    created_by  VARCHAR(64)  NOT NULL,
    deleted_at  DATETIME DEFAULT NULL,
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_contract_contracts_status (status, deleted_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='合同主表';
```

Flyway 历史表也加前缀避免共库冲突：`spring.flyway.table: contract_flyway_history`。开发期无生产数据，可大胆重建迁移集。

---

## 5. 让服务经网关可达 —— 5 处登记（Java 特有坑集中在这里）

> ⚠️ 这是 Java 服务和 Rust 服务差别最大、也最容易漏的地方。**5 处少一处，要么 CI 红，要么网关 502/404。**

### ① `docker-compose-services.yml` 加服务

在 `infrastructure/docker-compose-services.yml` 的服务列表（`volumes:` 段之前）追加：

```yaml
  contract-service:
    build:
      context: ..
      dockerfile: services/contract-service/Dockerfile
    ports:
      - "${CONTRACT_SERVICE_PORT_EXPORT:-8088}:8080"   # 宿主8088 → 容器8080
    labels:
      - "SERVICE_NAME=contract-service"
      - "SERVICE_ID=contract-service"
      - "SERVICE_PORT=8080"
      - "SERVICE_TAGS=backend,contract-service,java"
      - "SERVICE_8080_CHECK_HTTP=/contract-api/v1/health"
      - "SERVICE_8080_CHECK_INTERVAL=15s"
      - "SERVICE_8080_CHECK_TIMEOUT=3s"
    environment:
      TENANT_ID: "${TENANT_ID}"
      SPRING_DATASOURCE_URL: "${DATABASE_URL_JDBC}"
      SPRING_DATASOURCE_USERNAME: "${MYSQL_USER}"
      SPRING_DATASOURCE_PASSWORD: "${MYSQL_PASSWORD}"
      SPRING_PROFILES_ACTIVE: "${SPRING_PROFILES_ACTIVE:-dev}"
      OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
      JAVA_OPTS: "-XX:+UseG1GC -XX:MaxRAMPercentage=75.0"
    depends_on:
      mysql:
        condition: service_healthy
    networks:
      - superpowers
```

健康检查 URL 带 context-path：`/contract-api/v1/health`。

### ② `api-gateway.toml` 的 `[egress_map]` 显式登记（★ 最易漏）

**网关没有「服务名默认解析到 `同名:8080`」的兜底**。`router.rs` 对 `egress_map` 未命中的 service 直接返回 **502 Bad Gateway**（`no egress configured for service '...'`）。`src/config.rs` 里的 `default_egress_map()` 只在**完全不加载配置文件**时生效；docker 部署加载的是 `infrastructure/config/api-gateway/api-gateway.toml`，其 `[egress_map]` 会**整体替换**默认表。

所以即使容器内监听 8080，也**必须**在 `api-gateway.toml` 显式加一行：

```toml
[egress_map]
# ...现有 7 行 ...
"contract-service"      = { host = "contract-service",      port = 8080 }
```

> 改完确认 api-gateway 的 compose 把该 toml bind-mount 进容器（`/app/config/api-gateway.toml`），否则 override 不生效。

### ③ 新建路由文件 `routes/contract.router.toml`

新建 `infrastructure/config/api-gateway/routes/contract.router.toml`。**注意是 `[[route]]` 单数**（照抄现有 `prue_backend.router.toml` 的格式，不是 `[[routes]]`）。

> ★ **Java 特有坑**：`downstream` 必须带 context-path。网关转发 URL = `egress.host:port` + `downstream` **原样拼接**。Rust 服务没有 context-path，downstream 是裸路径（如 `/audit/operation/{rest}`）；Java 服务有 `/contract-api`，必须写进去，否则请求打到 Spring 的根 context 上 → 404。

```toml
[[route]]
upstream   = "/v1/contract/{rest}"
downstream = "/contract-api/v1/contract/{rest}"
service    = "contract-service"

[[route]]
upstream   = "/v1/contract"
downstream = "/contract-api/v1/contract"
service    = "contract-service"

[[route]]
upstream   = "/v1/contract/health"
downstream = "/contract-api/v1/health"
service    = "contract-service"
public     = true
```

对应 Controller `@RequestMapping("/v1/contract")`（context-path `/contract-api` 由 Spring 自动加，Controller 里不写）。

校验路由：

```bash
cargo run -p api-gateway -- validate --routes infrastructure/config/api-gateway/routes
```

### ④ 登记表前缀

`scripts/check-db-naming.js` 的前缀数组（约第 6 行）加 `'contract_'`：

```js
  'iam_', 'audit_', 'token_', 'ntf_', 'cfg_', 'file_', 'prue_', 'contract_',
```

同时在 `contracts/db-conventions.md` 的表名前缀映射表加一行 `contract-service → contract_`（保持文档与脚本一致）。

### ⑤ `.env` 补变量

`infrastructure/.env` 加（若缺）：

```dotenv
CONTRACT_SERVICE_PORT_EXPORT=8088
# JDBC 连接串（共享 superpowers 库），供 Java 服务用：
DATABASE_URL_JDBC=jdbc:mysql://mysql:3306/superpowers?useSSL=false&serverTimezone=UTC&allowPublicKeyRetrieval=true
```

---

## 6. 契约层（权限码 / 错误码）

`contracts/permissions.yaml` 加合同权限（格式 `<SERVICE>_<RESOURCE>_<ACTION>`，动作后缀 CREATE/READ/UPDATE/DELETE/EXPORT/IMPORT）：

```yaml
  # === 合同管理 ===
  CONTRACT_CONTRACT_CREATE:
    description: "创建合同"
  CONTRACT_CONTRACT_READ:
    description: "查看合同"
  CONTRACT_CONTRACT_UPDATE:
    description: "编辑合同"
  CONTRACT_CONTRACT_DELETE:
    description: "删除合同"
  CONTRACT_CONTRACT_EXPORT:
    description: "导出合同"
```

错误码加到 `contracts/error-codes.yaml`，遵循服务前缀约定。改完：

```bash
node scripts/contracts-gen.js          # 重新生成 Rust 侧 codes.rs / perms.rs（禁止手改生成文件）
```

> Java 服务侧的权限码字符串、错误信封与 YAML 保持一致即可；`GlobalExceptionHandler` 已按 `{code,message,path,timestamp,data}` 信封返回。

---

## 7. 调用平台能力（审计 / 通知 / 用户信息）

新业务服务（如合同）常常需要复用平台能力：**记审计、发通知、查用户信息、读动态配置、传文件**。这些都不直连下游服务端口，统一 **HTTP 走 api-gateway + Java SDK**。本节给 Spring Boot 3 下可直接用的接法。

> 更底层的「外部 Java 应用如何登录、怎么配 token」见 [Java 集成](./java-integration)；SDK 发布见 [Java SDK 发布](./java-sdk-publishing)。本节聚焦**本仓新服务**这一具体场景。

### 7.1 先分清：出站调用以谁的身份？

| 场景 | 身份 | token 来源 | 用哪套客户端 |
|---|---|---|---|
| 业务动作（用户在操作合同，要记审计/发通知给该用户） | **代表当前登录用户** | 透传入站请求的 `Authorization: Bearer <jwt>` | 旧式 facade（`AuditServiceClient` / `NotificationServiceClient`）配 bearer supplier |
| 系统级后台任务（定时扫描到期合同、批量建角色、服务自查用户） | **应用身份（M2M，无用户）** | OAuth client_credentials，SDK 自动换/缓存/刷新 | 新式 `XxxClient.create(baseUrl, apiKey, secret, code)` |
| 只要当前用户的 id / 用户名 / 角色 / 权限 / 租户 | 无需远程调用 | 直接从 JWT `Principal` 取 | 见 §7.5 |

**Java 服务不需要 `SERVICE_BOOTSTRAP_TOKEN`**（那是 Rust 服务间 service-jwt 用的）。Java 出站一律走网关。

前置：baseUrl 统一填**网关地址**（本地 `http://localhost:3000`，容器内 `http://api-gateway:3000`，生产换成网关域名）。

### 7.2 Maven 依赖

在 `contract-app-infrastructure/pom.xml` 按需放开（groupId 统一 `io.digua`，版本用父 POM 的 `${digua-sdk.version}`）：

```xml
<dependency>
  <groupId>io.digua</groupId>
  <artifactId>audit-service-sdk</artifactId>          <!-- 审计：AuditServiceClient -->
  <version>${digua-sdk.version}</version>
</dependency>
<dependency>
  <groupId>io.digua</groupId>
  <artifactId>notification-service-sdk</artifactId>   <!-- 通知：公告 facade -->
  <version>${digua-sdk.version}</version>
</dependency>
<dependency>
  <groupId>io.digua</groupId>
  <artifactId>user-service-client-sdk</artifactId>    <!-- 应用身份查用户/角色/组织 -->
  <version>${digua-sdk.version}</version>
</dependency>
<dependency>
  <groupId>io.digua</groupId>
  <artifactId>user-service-user-sdk</artifactId>      <!-- 用户身份查用户（少见） -->
  <version>${digua-sdk.version}</version>
</dependency>
<dependency>
  <groupId>io.digua</groupId>
  <artifactId>config-service-sdk</artifactId>         <!-- 动态配置 -->
  <version>${digua-sdk.version}</version>
</dependency>
<dependency>
  <groupId>io.digua</groupId>
  <artifactId>file-service-sdk</artifactId>           <!-- 附件/对象存储 -->
  <version>${digua-sdk.version}</version>
</dependency>
```

> 旧式 facade（审计 / 公告）底层用 OkHttp；新式 client（`UserServiceClientClient` 等，继承 `SdkClient`）底层用 `HttpClientWrapper` + `TokenCache`，401 会自动刷新重试一次。

### 7.3 让「当前请求的 JWT」可被 facade 透传

`AuditServiceClient` / `NotificationServiceClient` 构造时收一个 `Supplier<String>`，每次调用现取 bearer。在 Spring Boot 3 里用请求级 holder 传递：

```java
// 一个简单的请求级 token 持有器
public final class BearerTokenHolder {
    private static final ThreadLocal<String> TOKEN = new ThreadLocal<>();
    private BearerTokenHolder() {}
    public static void set(String t) { TOKEN.set(t); }
    public static String get()       { return TOKEN.get(); }
    public static void clear()       { TOKEN.remove(); }
}
```

在 `JwtAuthFilter`（见 §3）里，解码 JWT 的同时把**原始 token**存进来（解析成功后），请求结束清理：

```java
String raw = token;                       // Authorization: Bearer 后面那段
BearerTokenHolder.set(raw);
try {
    chain.doFilter(req, res);
} finally {
    BearerTokenHolder.clear();
}
```

然后把各客户端注册成 Spring Bean（`@Configuration`）：

```java
@Bean
public AuditServiceClient auditServiceClient(@Value("${digua.gateway.base-url}") String gw) {
    return new AuditServiceClient(gw, BearerTokenHolder::get);   // 代表当前用户
}

@Bean
public NotificationServiceClient notificationServiceClient(@Value("${digua.gateway.base-url}") String gw) {
    return new NotificationServiceClient(gw, BearerTokenHolder::get);
}

// 应用身份（后台任务）：新式 client，自动 client_credentials
@Bean
public UserServiceClientClient userServiceClientClient(@Value("${digua.gateway.base-url}") String gw,
                                                       KudiClientProps props) {
    return UserServiceClientClient.create(gw, props.getApiKey(), props.getApiSecret(), props.getCode());
}
```

配置走 `application.yml`（账号/密钥用环境变量注入，勿硬编码）：

```yaml
digua:
  gateway:
    base-url: ${DIGUA_GATEWAY_URL:http://api-gateway:3000}
  client:
    api-key: ${DIGUA_CLIENT_API_KEY:}
    api-secret: ${DIGUA_CLIENT_API_SECRET:}
    code: ${DIGUA_CLIENT_CODE:}
```

> `api-key / secret / code` 是在 token-service 注册的 OAuth client 凭证（见 [Java 集成](./java-integration) §鉴权）。代表用户的 facade 不需要这组凭证，透传用户 JWT 即可。

### 7.4 审计（AuditServiceClient）

合同的创建、审批、作废等敏感动作都应记审计。审计写失败**不应阻断主业务**——包一层软失败（参考 examples 的 `AuditPublisher`）：

```java
@Service
public class ContractAuditPublisher {
    private final AuditServiceClient client;
    public ContractAuditPublisher(AuditServiceClient client) { this.client = client; }

    public void publish(Principal actor, String action, String resourceId, Map<String, Object> payload) {
        AuditEvent ev = new AuditEvent(
                actor.userId() != null ? actor.userId().toString() : actor.username(),
                action,                 // 如 "CONTRACT_APPROVE"
                "contract",             // resourceType
                resourceId,
                payload,
                Instant.now());
        try {
            client.sendEvent(ev);       // 返回 AuditLog
        } catch (Exception e) {
            log.warn("audit publish failed action={} resource={}: {}", action, resourceId, e.getMessage());
        }
    }
}
```

查询审计日志（分页）：

```java
AuditLogQuery q = new AuditLogQuery();
q.setActor(userId);
q.setAction("CONTRACT_APPROVE");
q.setPage(1); q.setSize(20);
PageResult<AuditLog> logs = auditClient.queryLogs(q);
```

> 权限：写/查审计需要对应审计权限码，确保调用方 JWT（用户角色或 client 授权）包含它。

### 7.5 访问用户信息

**只要当前用户自己的身份 → 不用远程调用**，`JwtAuthFilter` 已把 claims 装进 Spring Security 的 `Principal`：

```java
Principal me = (Principal) SecurityContextHolder.getContext().getAuthentication().getPrincipal();
Long tenantId = me.tenantId();          // 多租户隔离必用
List<String> roles = me.roles();
List<String> perms = me.permissions();  // 配合 @PreAuthorize
String username = me.username();
Long userId  = me.userId();
```

**要查别的用户 / 角色 / 组织（应用身份）** → `UserServiceClientClient`：

```java
// 查用户
User u = userClient.getById(userId);
List<User> all = userClient.list();

// 角色 / 权限 / 组织
List<Role> roles = userClient.listRoles();
List<Permission> perms = userClient.listPermissions();
List<OrgUnit> orgs = userClient.listOrgUnits();
Role role = userClient.createRole(Map.of("name", "CONTRACT_MANAGER", "description", "合同管理员"));

// 内部批量查（需带 tenant_id，service-jwt 场景）
BatchGetUsersResponse batch = userClient.batchGetUsers(List.of("u1", "u2"), tenantId);
```

> `getById("me")` 可拿当前登录用户资料（配合用户身份 client）。跨用户、批量、组织类查询属于内部能力，用应用身份 client 并带 `tenantId`。

### 7.6 通知

通知有两条路径，按用途选：

**(a) 公告 / 站内信（管理端发布、用户读未读）** —— `NotificationServiceClient`（facade）：

```java
AnnouncementRequest req = new AnnouncementRequest(/* 标题、内容、置顶等，按模型字段填 */);
Announcement a = notificationClient.createAnnouncement(req);
notificationClient.publishAnnouncement(a.getId());          // 发布
long unread = notificationClient.unreadCount();             // 我的未读数
PageResult<Announcement> mine = notificationClient.listMyAnnouncements(false, 1, 20);
notificationClient.markAsRead(a.getId());
```

**(b) 发一条业务消息（如「合同待审批」推给指定人）** —— 直接 POST 通知服务的消息端点（examples 用 `KudiHttpClient`）：

```java
Map<String, Object> body = Map.of(
        "channel", "in_app",                 // in_app / email / sms ...
        "user_id", approverUserId,
        "template", "CONTRACT_PENDING_APPROVAL",
        "payload", Map.of("contractId", id, "title", title));
String url = gatewayBaseUrl + "/v1/ntf/notifications";
HttpTransport.Response resp = kudiHttpClient.post(url, objectMapper.writeValueAsString(body), BearerTokenHolder.get());
```

> 注意网关前缀差异：公告走 `/v1/notification/announcements`（facade 已封装），发消息走 `/v1/ntf/notifications`。模板号、渠道以 notification-service 的契约为准。

### 7.7 动态配置（config-service）

配置中心网关路由 `/v1/config/namespace/{ns}/*`（另有 SSE `/v1/config/watch`）。适合放开关、阈值、模板配置，改配置不发版。examples 的 `ConfigPoller` 每 5s 轮询某个命名空间的整包 values 到本地缓存，支持「等待配置变更」：

```java
// 轮询拉取命名空间配置（后台 @Scheduled，5s 一次）
String url = gatewayBaseUrl + "/v1/config/namespace/contract/values";  // {ns}=contract
HttpTransport.Response resp = kudiHttpClient.get(url, null);   // 应用身份可传 client token
if (resp != null && resp.body != null && resp.body.startsWith("{")) {
    // 解析扁平 JSON { "key": "value", ... } 进本地缓存
}
String approvalTimeoutDays = cache.get("approval.timeout.days");
```

> examples 历史代码里用的是 `/v1/cfg/namespaces/demo/values` 旧前缀；**当前网关权威前缀是 `/v1/config/...`**（以 `infrastructure/config/api-gateway/routes/config.router.toml` 为准）。需要实时推送而非轮询时，用 `/v1/config/watch` SSE（浏览器端 EventSource 走 `?token=` query）。

### 7.8 平台能力调用要点

- 全部经网关：baseUrl 填网关，**不要**直连 `iam-service:8081` / `audit-service:8084` 等容器端口（绕过网关就没有限流、追踪、JWT 校验）。
- 代表用户 → 透传请求 JWT；后台任务 → client_credentials（`XxxClient.create(...)`，SDK 自动管 token）。
- 审计等旁路写操作建议**软失败**（catch + warn 日志），不阻断主交易；需要强一致审计的场景再考虑失败回滚。
- 每次跨服务调用带上网关透传的 trace 上下文，方便在 Jaeger 串链路。
- 调用受权限保护的平台接口前，确认调用方（用户角色权限 或 client 授权范围）已授予对应权限码。

---

## 8. 附件 / 文件接入全链路（前端上传 → 业务后端 → 下载）

新业务（如合同附件、头像、单据扫描件）**不要自己存文件**。平台提供 `file-service`：统一对象存储、多租户隔离、权限码控制、临时文件过期。业务系统只在自己的表里存 **file id（UUID 字符串，`VARCHAR(64)`）** 做关联。

> ⚠️ **上传协议以 multipart 为准**：当前权威上传接口是 `POST /v1/file/upload`，`multipart/form-data`，字段 `metadata`（JSON）+ `file`（字节）。早期 examples 里的 base64 `POST /v1/file/files` 写法已与现行路由漂移，**勿照抄**。

### 8.1 推荐架构：前端直传 file-service，业务后端只存 id

```
浏览器                    api-gateway              file-service          业务服务(contract)
  │  ① multipart 上传  ───────────────▶  /v1/file/upload ──▶ 对象存储+file_files
  │  ◀─────────────  返回 { id, filename, size, ... }
  │  ② 业务提交合同 { ..., attachmentFileId: id } ──▶ /v1/contract  ──▶ contract_contracts 存 id
  │  ③ 展示/下载     <img src=fileUrl(id)> 或 GET /v1/file/{id} ──▶ file-service 回源
```

文件字节**不经过** contract-service —— 业务服务零文件 IO、零大 body 内存压力，存储/限额/权限全在 file-service。这是平台前端（`frontend/src/api/file.ts`）使用的模式。

### 8.2 file-service 接口速查（经网关 `/v1/file/*`）

| 方法 | 路径 | 说明 | 权限码 |
|---|---|---|---|
| POST | `/v1/file/upload` | multipart 上传（`metadata` JSON + `file`），返回 `FileInfo{ id, filename, content_type, size, ... }` | `FILE_UPLOAD` |
| GET | `/v1/file/{id}` | 下载文件字节流（带 Content-Type / Content-Disposition） | `FILE_DOWNLOAD` |
| GET | `/v1/file/{id}/info` | 查文件元数据 | `FILE_READ` |
| DELETE | `/v1/file/{id}` | 删除（对象存储 + 记录） | `FILE_DELETE` |
| GET | `/v1/file/list` | 分页列表（可按 tenant_id/user_id/filename） | `FILE_READ` |
| POST | `/v1/file/presign` | 取预签名 URL（`operation=get/put`，默认 3600s），用于 `<img>`/直传/外链 | `FILE_READ` |

关键事实（`services/file-service/src/`）：

- **上传 meta**：`metadata` JSON 支持 `filename`、`content_type`、`is_temporary`、`expires_in_seconds`。`is_temporary=true` + `expires_in_seconds`（前端 60–86400）= 临时文件，到期自动失效，适合「先上传、表单还没提交」的场景。
- **大小硬限 100MB**（`file-service/src/config.rs`）。前端在 `before-upload` 就要拦截超限文件，避免无意义上传。
- **多租户隔离**：file-service 从 JWT 取 `tenant_id` 注入查询，跨租户访问不到；下载/详情/删除都带租户 scope。
- **存储**：对象存储抽象（本地 / S3 兼容，`storage/s3.rs`），存储路径 `{file_id}-{filename}`；业务侧无需关心落盘位置。

### 8.3 前端：上传 / 展示 / 下载

平台前端已有封装 `frontend/src/api/file.ts`（axios，自动带 JWT）。新业务前端直接用同一套：

```ts
import { uploadFile, downloadFile, fileUrl } from "@/api/file";

// ① 上传（el-upload / input[type=file] 拿到 File 对象）
const meta = await uploadFile(file, {
  content_type: file.type || "application/octet-stream",
  is_temporary: true,          // 表单未提交前先作临时文件
  expires_in_seconds: 86400
});
const fileId = meta.id;        // 拿这个 id 随业务表单提交

// ② 提交业务数据时把 fileId 一起发给合同服务
await createContract({ title, attachmentFileId: fileId });

// ③a 图片预览 / <img>：用 fileUrl() 拼绝对路径（<img> 不走 axios baseURL）
<img :src="fileUrl('/v1/file/' + fileId)" />

// ③b 触发下载
await downloadFile(fileId);    // 或 <a :href="fileUrl('/v1/file/'+id)" target="_blank">
```

前端守卫（`frontend/src/api/file/limits.ts`，新页面沿用）：上传前 `checkFileSize(<=100MB)`、`checkFileExtension`（拦截 `.exe/.bat/.sh/.js` 等高危扩展名，纯 UX 守卫、后端不强制）。按钮用 `hasPerms(['FILE_UPLOAD'])` 控制可见。

> 为什么 `<img>` 要 `fileUrl()`：后端返回的是相对路径 `/v1/file/<id>`，axios 会自动加 baseURL 但 `<img src>` 不会；`fileUrl()` 用 `VITE_API_BASE`（默认 `/api`）拼成绝对 URL。预签名 URL（`http(s)://…`）会被原样返回。

### 8.4 业务后端（Java）：存 id、做关联、返回信息

合同表只存 file id，不存文件本身：

```sql
-- V1__init.sql 里
ALTER TABLE contract_contracts
  ADD COLUMN attachment_file_id VARCHAR(64) NULL COMMENT '附件 file id',
  ADD KEY idx_contract_attachment (attachment_file_id);
-- file id 是 UUID 字符串 → VARCHAR(64)，严禁 BIGINT
```

Controller 接收前端提交的 fileId 并落库（权限用合同自己的权限码，如 `CONTRACT_CONTRACT_UPDATE`）：

```java
@PostMapping("/v1/contract")
@PreAuthorize("hasAuthority('CONTRACT_CONTRACT_CREATE')")
public ContractDto create(@RequestBody @Valid CreateContractRequest req, Principal principal) {
    // req.attachmentFileId() 即前端直传 file-service 拿到的 id
    return contractService.create(req, principal);
}
```

**后端不转发文件字节**。需要元数据/下载时，让前端直接打 `/v1/file/*`（带用户 JWT，网关鉴权 + 租户隔离）。只有在「服务端必须处理文件内容」（如后端解析 PDF、生成缩略图、跨系统同步）时才由服务端拉取，此时用服务端 HTTP 客户端调网关的 **multipart 上传**接口（不是 base64）：

```java
// 服务端 multipart 上传（仅在确需服务端持有时用；日常让前端直传）
MultiValueMap<String, Object> body = new LinkedMultiValueMap<>();
body.add("metadata", objectMapper.writeValueAsString(Map.of(
        "filename", file.getOriginalFilename(),
        "content_type", file.getContentType())));
body.add("file", new ByteArrayResource(file.getBytes()) {
    @Override public String getFilename() { return file.getOriginalFilename(); }
});
HttpHeaders h = new HttpHeaders();
h.setContentType(MediaType.MULTIPART_FORM_DATA);
h.setBearerAuth(BearerTokenHolder.get());           // 代表当前用户；后台任务用 client token
String resp = restClient.post()
        .uri(gatewayBaseUrl + "/v1/file/upload")
        .headers(hh -> hh.addAll(h))
        .body(body)
        .retrieve().body(String.class);
String fileId = objectMapper.readTree(resp).path("id").asText();
```

> 说明：Java 侧目前没有独立的 file 高层 SDK artifact（`file-service-sdk` 主要面向其它场景），examples 也是用通用 HTTP 客户端调网关。照上面 multipart 写法即可，路径/字段以 §8.2 为准。

### 8.5 下载与外链

- **站内下载/预览**：前端直接 `GET /v1/file/{id}`（带 JWT），file-service 回源对象存储并返回字节流；图片用 `fileUrl()` 给 `<img>`。
- **临时外链 / 第三方系统取文件**：`POST /v1/file/presign` 拿限时 URL（`{operation:"get", file_id, expires_in_seconds}`，默认 1 小时），URL 自带过期参数，可发给不带平台 JWT 的一方。
- **大文件直传**：`presign` 的 `operation:"put"` 给前端/客户端一个直传对象存储的 URL，文件不过网关、不过业务服务，适合超大附件。

### 8.6 附件链路要点

- 业务表只存 **file id（`VARCHAR(64)`）**；文件字节、存储、限额、租户隔离全在 file-service。
- **默认前端直传**（`POST /v1/file/upload` multipart），业务后端不中转；服务端仅在必须处理文件内容时才调 multipart 接口。
- 上传用 multipart（`metadata` + `file`），**不要用旧的 base64 `/v1/file/files`**。
- 权限码：上传 `FILE_UPLOAD`、下载 `FILE_DOWNLOAD`、读信息 `FILE_READ`、删除 `FILE_DELETE`——在角色/权限里授予对应用户或 client。
- 临时文件：`is_temporary=true` + `expires_in_seconds`，表单正式提交后建议由业务侧把它转为正式关联（或上传时就不带临时标记）。
- 前端 100MB 硬限 + 高危扩展名校验在 `before-upload` 拦截。

---

## 9. 验证门禁（push 前全过）

```bash
# 1. 服务规范基线（期望 contract-service 8/8，脚本通过 pom.xml 识别 Java 服务）
node scripts/check-service-standard.js

# 2. 表名前缀
node scripts/check-db-naming.js

# 3. 契约规范（权限码 / 错误码 / OpenAPI）
node scripts/check-spec.js

# 4. 契约代码无漂移
node scripts/contracts-gen.js --check

# 5. Maven 构建（需 JDK 21 + Maven 3.9+）
cd services/contract-service
mvn -Pjava21 clean package -DskipTests

# 6. 网关路由校验
cargo run -p api-gateway -- validate --routes infrastructure/config/api-gateway/routes

# 7. Docker 构建（在仓库根执行，构建上下文是根）
docker build -t contract-service:0.1.0 -f services/contract-service/Dockerfile .
```

联调验证（起栈后）：

```bash
# 宿主直连服务（带 context-path）
curl http://localhost:8088/contract-api/v1/health

# 经网关（对外路径，不带 context-path）
curl http://localhost:3000/v1/contract/health          # public 路由
curl -H "Authorization: Bearer <jwt>" \
     http://localhost:3000/v1/contract/                # 受保护路由

# Swagger
#   直连：http://localhost:8088/contract-api/swagger-ui/index.html
```

---

## 10. 常见坑速查

| 现象 | 根因 | 解决 |
|---|---|---|
| 网关返回 **502** `no egress configured for service 'contract-service'` | 漏了 §5② egress_map 登记（网关无 8080 兜底） | 在 `api-gateway.toml` `[egress_map]` 加 `"contract-service" = { host="contract-service", port=8080 }` |
| 网关通但下游 **404** | `downstream` 漏了 context-path | downstream 写成 `/contract-api/v1/contract/{rest}`（§5③） |
| 健康检查一直 unhealthy | 健康 URL 没带 context-path | label 用 `/contract-api/v1/health`（§5①） |
| CI `check-db-naming` 红 | 表前缀没登记 | `scripts/check-db-naming.js` + `contracts/db-conventions.md` 都加 `contract_`（§5④） |
| 启动报 mapper 找不到 | 漏 `@MapperScan` | `ContractApplication` 加 `@MapperScan("com.coder.contract.infra.persistence.mapper")` |
| 迁移历史表冲突 | 共库 Flyway 历史表重名 | `spring.flyway.table: contract_flyway_history` |
| 用户 id 列插入失败/截断 | 用了 `BIGINT` 存用户 id | 用户引用列一律 `VARCHAR(64)`（§3）；file id 也是 `VARCHAR(64)`（§8） |
| 附件上传 404 / 参数解析失败 | 用了旧的 base64 `POST /v1/file/files` | 改 multipart `POST /v1/file/upload`（字段 `metadata`+`file`），见 §8.2 |
| `<img>` 附件预览裂图 | `<img src>` 不走 axios baseURL，相对路径解析不到网关 | 用 `fileUrl('/v1/file/'+id)` 拼 `VITE_API_BASE`（§8.3） |
| Docker 构建 Stage 1 找不到 `sdk/java/` | 构建上下文不是仓库根 | compose `build.context: ..`，`docker build` 在仓库根执行 |
| 本地 `cargo run` 网关调不到容器服务 | 宿主/容器网络差异 | 网关也跑容器内（compose 网络），或本地联调直接 curl 宿主端口 |
| 路由文件改了不生效 | 配置未重载 | `cargo run ... validate` 后重启网关，或 `POST /__admin/reload` 热加载 |

---

## 11. 交付检查清单

- [ ] `services/contract-service/` 4 模块 Maven 工程，`mvn -Pjava21 package` 通过
- [ ] 8 项基线齐全：Dockerfile / config / README / AGENTS / health（liveness+readiness）/ Flyway 迁移 / Swagger / tests
- [ ] `server.port=8080` + `context-path=/contract-api`
- [ ] compose 服务定义（含带 context-path 的健康检查 label）
- [ ] `api-gateway.toml` `[egress_map]` 登记 contract-service:8080
- [ ] `contract.router.toml`（`[[route]]` 单数，downstream 带 `/contract-api`）+ `validate` 通过
- [ ] 表前缀 `contract_` 登记到 `check-db-naming.js` 与 `db-conventions.md`
- [ ] 权限码 / 错误码进 YAML 并 `contracts-gen.js`
- [ ] `.env` 端口与 JDBC 变量
- [ ] 用户引用列均为 `VARCHAR(64)`
- [ ] 附件：业务表只存 `*_file_id VARCHAR(64)`；前端直传 `POST /v1/file/upload`（multipart），未用旧 base64 `/v1/file/files`；下载/预览走 `/v1/file/{id}`（`FILE_*` 权限已授）
- [ ] 出站平台能力（审计/通知/用户/配置）走网关 + SDK，未直连下游端口
- [ ] 5 个 CI 门禁 + Maven 构建 + 网关 validate + Docker 构建全过
