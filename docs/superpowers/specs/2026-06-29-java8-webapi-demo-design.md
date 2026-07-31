# Java 8 WebAPI 集成示例设计（examples/java8-webapi）

> Spec ID: java8-webapi-demo
> Status: **Approved (2026-07-06 完成收尾)**
> Created: 2026-06-29
> Plan: `docs/superpowers/plans/2026-06-29-java8-webapi-demo-plan.md`
> 完成记录：17 个 commit（2026-06-29 ~ 2026-06-30），30 个 unit/MockMvc 测试全过（audit-service-sdk 4 + demo-shared 4 + demo-webapp 22），`mvn -Pjava8 install` 成功。

## 1. 概述

### 1.1 技术栈：Spring Boot 2.1.18.RELEASE 与 1.5 的差异

- Spring Framework 5.1.x（1.5 是 4.3.x），仍支持 Java 8 / 11
- 命名空间仍为 `javax.*`（Jakarta EE 迁移要到 Spring Boot 3.0）
- 默认连接池换成 HikariCP（替换 tomcat-jdbc）
- Hibernate 升到 5.3（JPA 2.2）
- `spring-boot-starter-test` 默认含 JUnit 5（1.5 默认 JUnit 4）
- Spring Security 5.x（1.5 是 4.2）
- `WebMvcConfigurerAdapter` 弃用，直接实现 `WebMvcConfigurer` 即可

### 1.2 背景

`coder-micro-services` 是 Rust 微服务平台（iam-service / token-service / audit-service / config-service / file-service / notification-service / api-gateway），已有 `sdk/java/` 客户端 SDK（digua_common + 4 个用户/客户端 SDK + kudi-http-client）。

但**仓库里没有一个 Java 8 业务服务的参考实现**：新人 / 客户想知道"用 SDK 接入这个平台要写多少代码、长什么样"时，没有可跑的样例。`tests/e2e` 现有 Node.js 端到端测试，但 Java 8 这条线只到 SDK 层，缺一个端到端 webapi 入口。

### 1.3 目标

新建 `examples/java8-webapi/`（Spring Boot 2.1.18.RELEASE + Java 8 多模块 Maven 工程）作为**集成示例 / Demo**，覆盖 6 类能力：
1. 安全/认证（登录、JWT 获取）
2. 权限校验（user / client / resource 3 种鉴权场景）
3. 审计（上报业务事件 + 查询日志）
4. 配置中心（动态读取 + 热更新）
5. 通知服务（发送通知）
6. 文件服务（上传 / 下载）

并**顺手补一个 Java 8 SDK 缺位**：`sdk/java/audit-service-sdk/`（platform 已有 audit-service 但无对应 Java SDK，本次一并实现并发布到本地 `~/.m2`）。

### 1.4 显式排除（YAGNI）

- 不做 Docker 镜像（`mvn spring-boot:run` 跑通即可）
- 不做 Swagger UI（用 curl + README）
- 不做 OTel / Prometheus（demo 重点是 SDK 集成）
- 不做多租户、复杂任务协作（评论、标签、看板）
- 不做 token 自动刷新拦截器（demo 中手动调一次 refresh 演示即可）
- 不做 system / Docker Compose 编排（demo 跑得起来的前提是 7 个 Rust 服务已起来，复用现有 `pnpm run dev:infra`）
- 不做 CI 集成、不做覆盖率报告
- 不做 audit-service-sdk 的发布到内网 Maven 仓库（仅 install 到 `~/.m2` 供 demo 引用）

## 2. 架构

### 2.1 模块结构（多模块 Maven）

```
sdk/java/
├── ... 现有 6 个 Java SDK 模块
└── audit-service-sdk/        # ★ 新增：手写的 Java 8 SDK
    ├── pom.xml
    └── src/main/java/com/kudi/coder/auditsdk/
        ├── AuditServiceClient.java
        ├── model/{AuditEvent,AuditLog,PageResult}.java
        └── exception/{AuditException,AuditAuthException}.java

examples/java8-webapi/        # ★ 新增：Spring Boot 1.5.x demo
├── pom.xml                   # 父 POM（Java 8 + Spring Boot 2.1.18.RELEASE）
├── README.md                 # 跑通教程
├── scripts/demo.sh           # 15 个 curl 的演示脚本
├── demo-shared/              # 跨模块共享
│   ├── pom.xml
│   └── src/main/java/com/kudi/coder/demo/shared/
│       ├── ApiResponse.java           # {code,message,data,request_id}
│       ├── ErrorCode.java             # 错误码常量
│       └── PageReq.java
└── demo-webapp/              # Spring Boot 2.1.18 主程序
    ├── pom.xml
    └── src/main/java/com/kudi/coder/demowebapp/
        ├── DemoApplication.java
        ├── web/
        │   ├── AuthDemoController.java         # 1 登录 / 2 刷新 / 3 撤销
        │   ├── TaskController.java             # 业务主轴：CRUD
        │   ├── PermissionDemoController.java   # 1 user / 2 client / 3 resource
        │   ├── AuditDemoController.java        # 1 上报 / 2 查询
        │   ├── ConfigDemoController.java       # 1 读 / 2 热更新演示
        │   ├── NotificationDemoController.java # 1 发通知
        │   └── FileDemoController.java         # 1 上传 / 2 下载
        ├── service/TaskService.java
        ├── repository/TaskRepository.java      # Spring Data JPA + H2
        ├── domain/Task.java                    # 工单实体
        ├── security/
        │   ├── JwtAuthInterceptor.java         # 鉴权拦截器
        │   └── RequiresPermission.java         # 权限注解
        ├── security/PermissionAspect.java      # 注解 AOP
        ├── audit/AuditPublisher.java           # 同步软失败审计上报
        └── config/
            ├── KudiCoderProperties.java        # 配置项
            └── SdkConfig.java                  # @Bean: AuditServiceClient / KudiHttpClient
        └── src/main/resources/
            ├── application.yml
            └── data.sql                        # 工单 seed
```

### 2.2 6 类能力 ↔ SDK 映射

| 能力 | 演示端点 | 用的 SDK |
|---|---|---|
| ① 安全/认证 | `POST /demo/auth/login` 调 iam；`POST /demo/auth/refresh` 调 token-service | `user-service-user-sdk` + `token-server-user-sdk` |
| ② 权限校验 | `POST /demo/perm/me` (user 端)、`POST /demo/perm/internal` (client 端)、`GET /demo/perm/resource` (resource 令牌) | `user-service-user-sdk` + `user-service-client-sdk` + `token-server-user-sdk` |
| ③ 审计 | `POST /demo/audit/event` 上报；`GET /demo/audit/logs?actor=...` 查 | **audit-service-sdk（手写）** |
| ④ 配置中心 | `GET /demo/config/show` 当前值；后台线程每 5s 拉一次并 log diff | `kudi-http-client` 调 config-service |
| ⑤ 通知 | `POST /demo/notify/task-done` 发通知 | `kudi-http-client` 调 notification-service |
| ⑥ 文件 | `POST /demo/file/upload`（multipart）；`GET /demo/file/{id}` 下载 | `kudi-http-client` 调 file-service |

### 2.3 业务主题

工单系统。Task 实体字段：

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | Long | 主键自增 |
| `title` | String | 标题（必填） |
| `description` | String | 描述（可选） |
| `status` | enum | `CREATED` / `ASSIGNED` / `IN_PROGRESS` / `DONE` |
| `assigneeId` | String | 受让人 user_id（String，遵循平台雪花 i64 转 string 约定） |
| `createdAt` / `updatedAt` | Instant | ISO8601 UTC |

权限码定义：

| 权限码 | 含义 |
|---|---|
| `TASK_VIEW` | 读任务 |
| `TASK_CREATE` | 创建任务 |
| `TASK_ASSIGN` | 分配任务（修改 assigneeId） |
| `TASK_DELETE` | 删除任务 |
| `TASK_INTERNAL_ADMIN` | 内部管理端点专用（演示 client-sdk 鉴权） |

## 3. 关键设计决策

### 3.1 权限校验：拦截器 + 注解混合（推荐）

- **拦截器 `JwtAuthInterceptor`**：解析 `Authorization: Bearer <token>`，调用 `user-service-user-sdk` 验证 token，把 `userId` + `permissions` 塞进 `request.setAttribute("principal", ...)`；无 token 或无效 → 401（白名单 `/demo/auth/login`、`/demo/perm/public`）。
- **注解 `@RequiresPermission("TASK_ASSIGN")`** + AOP 切面 `PermissionAspect`：方法上标注权限码，AOP 在 method.invoke 前检查 principal.permissions 是否包含，不足 → 403。

理由：拦截器统一做"鉴权 + 取用户身份"，注解提供端点级粒度。两者各司其职，避免一个端点改权限要改 2 个文件。

### 3.2 审计：同步内联 + try/catch 软失败（推荐）

- `TaskService` 的 `create/assign/changeStatus/delete` 业务方法中，**业务主流程完成 → 调 `AuditPublisher.publish(event)` 同步上报**。
- `AuditPublisher` 内部 `try { auditClient.sendEvent(event) } catch (Exception e) { log.warn("audit send failed", e) }`，失败不抛。
- 不做 AOP `@Audited` 注解（反射 + 多 AOP 类，demo 价值低）；不做 `@Async` 队列（链路长，看不见）。

理由：demo 友好、调用栈清晰可读；生产场景后续可加异步队列替换 `AuditPublisher` 实现，不动业务。

### 3.3 配置中心热更新：5 秒轮询线程

- 启动时 `@PostConstruct` 起一个 `ScheduledExecutorService` 单线程，每 5s 调一次 config-service。
- 维护一个 `ConcurrentHashMap<String, String> configCache`；对比旧值，diff 打印 log。
- 端点 `GET /demo/config/show?key=foo` 直接读 cache；`GET /demo/config/wait-change?key=foo&expected=bar&timeoutSec=30` 阻塞等待值变化（演示用，便于 README 截图）。

理由：5s 轮询足够 demo 看见；用阻塞等待端点让"配置变了"成为可观察事件。生产改 push / long-poll 不属于本 demo 范围。

### 3.4 通知 / 文件：走 kudi-http-client

- `KudiHttpClient` 在 `sdk/java/kudi-http-client` 已有，是纯 JWT 透传 HTTP 客户端。
- `SdkConfig` 注册 `@Bean KudiHttpClient`，构造时传 baseUrl；端点处用 `kudiHttpClient.post("/v1/notify/...", body, bearer)`。

理由：notification / file / config 都是通知/资源型，不强求强类型 SDK；少写 3 个 SDK。

### 3.5 响应格式（严格遵循 `contracts/http-conventions.md`）

所有端点统一返回：

```json
{
  "code": "OK",
  "message": "success",
  "data": { ... },
  "request_id": "abc-123"
}
```

- 成功 `code = "OK"`，业务数据放 `data`。
- 业务异常 `code = "DEMO_TASK_3001"`（自定义前缀 `DEMO_`），`data = null`。
- 全局 `@RestControllerAdvice` 兜底：捕获 SDK 异常 → 错误码 + 错误消息。
- `request_id` 由 `JwtAuthInterceptor` 从上游 `X-Request-Id` 头透传（无则 UUID 生成）。

### 3.6 错误码（仅 demo 用最小集）

| 错误码 | 含义 | HTTP |
|---|---|---|
| `OK` | 成功 | 200 |
| `DEMO_AUTH_1001` | 凭据错 | 401 |
| `DEMO_AUTH_1002` | token 无效/过期 | 401 |
| `DEMO_PERM_2001` | 权限不足 | 403 |
| `DEMO_TASK_3001` | 任务不存在 | 404 |
| `DEMO_TASK_3002` | 任务状态非法 | 409 |
| `DEMO_VALID_4001` | 参数校验失败 | 400 |
| `DEMO_UPSTREAM_5001` | 上游服务异常 | 502 |

错误码常量集中在 `demo-shared` 的 `ErrorCode.java`，避免散落。

### 3.7 数据存储：H2 内存 + Spring Data JPA

- `spring.datasource.url=jdbc:h2:mem:demo;DB_CLOSE_DELAY=-1`
- `spring.jpa.hibernate.ddl-auto=create-drop`
- `data.sql` seed 3 条任务

理由：跨服务 demo 不需要持久化；H2 启动自动建表、零配置；关闭 demo JVM 自动清理。

## 4. audit-service-sdk 设计

### 4.1 入口

```java
public class AuditServiceClient {
    public AuditServiceClient(String baseUrl, Supplier<String> bearerSupplier);
    public AuditServiceClient(String baseUrl, Supplier<String> bearerSupplier, int timeoutMs);

    public AuditLog sendEvent(AuditEvent event);                  // POST /v1/audit/events
    public PageResult<AuditLog> queryLogs(AuditLogQuery q);       // GET  /v1/audit/logs
    public List<String> listEventTypes();                         // GET  /v1/audit/event-types
}
```

### 4.2 模型

- `AuditEvent`：actor, action, resourceType, resourceId, payload(Map<String,Object>), occurredAt(Instant)
- `AuditLog`：id, actor, action, resourceType, resourceId, payload, occurredAt
- `AuditLogQuery`：actor, action, resourceType, resourceId, page, size
- `PageResult<T>`：items, total, page, size

### 4.3 异常

- `AuditException`：基类
- `AuditAuthException extends AuditException`：401
- `AuditRateLimitException extends AuditException`：429

### 4.4 传输

- 复用 `sdk/java/digua_common` 的 `HttpClient`（OkHttp 4.x 或 HttpURLConnection，参考 `kudi-http-client` 选择）
- JSON 用 Jackson 2.16.1（与现有 SDK 对齐）
- `bearerSupplier` 模式：调用方每次发请求时调用一次（便于 token 刷新场景）

### 4.5 pom 关键依赖

```xml
<dependencies>
  <dependency>
    <groupId>com.kudi.coder</groupId>
    <artifactId>kudi-coder-sdk-digua_common</artifactId>
    <version>0.2.0</version>
  </dependency>
  <dependency>
    <groupId>com.squareup.okhttp3</groupId>
    <artifactId>okhttp</artifactId>
    <version>4.12.0</version>
  </dependency>
  <dependency>
    <groupId>com.fasterxml.jackson.core</groupId>
    <artifactId>jackson-databind</artifactId>
    <version>2.16.1</version>
  </dependency>
  <dependency>
    <groupId>com.fasterxml.jackson.datatype</groupId>
    <artifactId>jackson-datatype-jsr310</artifactId>
    <version>2.16.1</version>
  </dependency>
  <dependency>
    <groupId>org.junit.jupiter</groupId>
    <artifactId>junit-jupiter</artifactId>
    <scope>test</scope>
  </dependency>
  <dependency>
    <groupId>com.squareup.okhttp3</groupId>
    <artifactId>mockwebserver</artifactId>
    <version>4.12.0</version>
    <scope>test</scope>
  </dependency>
</dependencies>
```

### 4.6 测试

`AuditServiceClientTest`（MockWebServer 起本地 server，注入 dispatcher）：

1. `sendEvent_returns_log_on_200` —— 上报 200，返回 AuditLog
2. `queryLogs_parses_pagination` —— 查询分页解析
3. `sendEvent_throws_AuthException_on_401` —— 凭据错
4. `sendEvent_throws_AuditException_on_500` —— 上游错

## 5. demo-webapp 关键 Bean / 配置

### 5.1 application.yml

```yaml
server:
  port: 7000
spring:
  datasource:
    url: jdbc:h2:mem:demo;DB_CLOSE_DELAY=-1
    driver-class-name: org.h2.Driver
    username: sa
    password: ""
  jpa:
    hibernate.ddl-auto: create-drop
    show-sql: false
    properties:
      hibernate.format_sql: false
kudi:
  gateway:
    base-url: ${KUDI_GATEWAY_URL:http://localhost:3000}
  iam:
    username: ${DEMO_USER:demo-user}
    password: ${DEMO_PASS:demo-pass}
  client:
    api-key: ${DEMO_API_KEY:demo-key}
    api-secret: ${DEMO_API_SECRET:demo-secret}
```

### 5.2 SdkConfig.java

```java
@Configuration
public class SdkConfig {
    @Bean
    AuditServiceClient auditClient(KudiCoderProperties p) {
        return new AuditServiceClient(
            p.getGateway().getBaseUrl(),
            () -> p.getIam().getStaticBearer()
        );
    }
    @Bean
    KudiHttpClient kudiHttpClient(KudiCoderProperties p) {
        return new KudiHttpClient(p.getGateway().getBaseUrl());
    }
    @Bean
    RestTemplate restTemplate() {
        RestTemplate rt = new RestTemplate();
        rt.getInterceptors().add((req, body, ex) -> {
            String rid = MDC.get("requestId");
            if (rid != null) req.getHeaders().add("X-Request-Id", rid);
            return ex.execute(req, body);
        });
        return rt;
    }
}
```

### 5.3 JwtAuthInterceptor

- `preHandle`：
  1. 路径白名单 `/demo/auth/login`、`/demo/perm/public` → 放行
  2. 取 `Authorization: Bearer <token>`；无 → 401
  3. 调 `userServiceClient.verifyToken(token)`，失败 → 401
  4. 把 `principal = {userId, permissions[]}` 塞进 `request.setAttribute`
  5. 透传/生成 `X-Request-Id`，写 MDC
- `afterCompletion`：清 MDC

## 6. 演示脚本 `examples/java8-webapi/scripts/demo.sh`

15 个 curl 按 6 类能力串场：

```
1. POST /demo/auth/login              # 拿 access_token + refresh_token
2. POST /demo/auth/refresh            # 刷 access_token
3. POST /demo/perm/me                 # user-sdk：读"我"
4. POST /demo/perm/internal           # client-sdk：内部端点（用 client token）
5. GET  /demo/perm/resource?res=task:42  # resource token 验证
6. POST /demo/audit/event             # 上报"任务创建"事件
7. GET  /demo/audit/logs?actor=alice  # 查审计日志
8. GET  /demo/config/show?key=foo     # 读当前配置
9. (后台开 5s 轮询，tail demo.log 看 diff)
10. POST /demo/file/upload            # 上传附件
11. GET  /demo/file/{id}              # 下载
12. POST /demo/notify/task-done       # 发通知
13. POST /demo/tasks                  # 创建任务（需 TASK_CREATE）
14. POST /demo/tasks/{id}/assign      # 分配任务（需 TASK_ASSIGN）
15. POST /demo/auth/revoke            # 撤销 token
```

## 7. 测试

| 范围 | 测试 | 用例数 |
|---|---|---|
| `audit-service-sdk` | `AuditServiceClientTest`（MockWebServer） | 4 |
| `demo-webapp` 7 个 controller 各 1 个 `MockMvc` 烟雾测试 | — | 7 |
| `demo-shared` | 不测试 | 0 |

**不写**：
- `@SpringBootTest` 全量上下文（慢）
- 契约测试（已有 `scripts/contracts-gen.js` 体系覆盖）
- 并发 / 性能 / 覆盖率

## 8. 验收标准

> 全部已落地（2026-07-06 收尾时复核）：
> - `cd sdk/java && mvn -Pjava8 -pl audit-service-sdk -am install` 成功，jar 进 `~/.m2` ✅
> - `cd examples/java8-webapi && mvn -Pjava8 clean install` 全模块通过 ✅
> - 30 个 unit/MockMvc 测试全过（audit-service-sdk 4 + demo-shared 4 + demo-webapp 22，含 commit `72261ba` 加的 TASK_VIEW 测试）✅
> - `mvn -Pjava8 -pl demo-webapp spring-boot:run` 启动，端口 7000 监听 ✅
> - `bash examples/java8-webapi/scripts/demo.sh` 15 个 curl 全过 ✅（需前置 Rust 服务栈）
> - `audit-service-sdk` 4 个单元测试全过 ✅
> - README 跑通教程，新人 5 分钟内可跑完 ✅

## 9. 文件清单（落盘清单）

**新增**：

- `sdk/java/audit-service-sdk/pom.xml`
- `sdk/java/audit-service-sdk/src/main/java/com/kudi/coder/auditsdk/AuditServiceClient.java`
- `sdk/java/audit-service-sdk/src/main/java/com/kudi/coder/auditsdk/model/{AuditEvent,AuditLog,AuditLogQuery,PageResult}.java`
- `sdk/java/audit-service-sdk/src/main/java/com/kudi/coder/auditsdk/exception/{AuditException,AuditAuthException,AuditRateLimitException}.java`
- `sdk/java/audit-service-sdk/src/test/java/com/kudi/coder/auditsdk/AuditServiceClientTest.java`
- `sdk/java/pom.xml` 追加 `<module>audit-service-sdk</module>`
- `sdk/java/README.md` 追加 audit-service-sdk 模块说明
- `examples/java8-webapi/pom.xml`（父 POM）
- `examples/java8-webapi/README.md`
- `examples/java8-webapi/scripts/demo.sh`
- `examples/java8-webapi/demo-shared/pom.xml`
- `examples/java8-webapi/demo-shared/src/main/java/com/kudi/coder/demo/shared/{ApiResponse,ErrorCode,PageReq}.java`
- `examples/java8-webapi/demo-webapp/pom.xml`
- `examples/java8-webapp/demo-webapp/src/main/java/com/kudi/coder/demowebapp/DemoApplication.java`
- `examples/java8-webapp/demo-webapp/src/main/java/com/kudi/coder/demowebapp/web/{AuthDemoController,TaskController,PermissionDemoController,AuditDemoController,ConfigDemoController,NotificationDemoController,FileDemoController}.java`
- `examples/java8-webapp/demo-webapp/src/main/java/com/kudi/coder/demowebapp/service/TaskService.java`
- `examples/java8-webapp/demo-webapp/src/main/java/com/kudi/coder/demowebapp/repository/TaskRepository.java`
- `examples/java8-webapp/demo-webapp/src/main/java/com/kudi/coder/demowebapp/domain/Task.java`
- `examples/java8-webapp/demo-webapp/src/main/java/com/kudi/coder/demowebapp/security/{JwtAuthInterceptor,RequiresPermission,PermissionAspect}.java`
- `examples/java8-webapp/demo-webapp/src/main/java/com/kudi/coder/demowebapp/audit/AuditPublisher.java`
- `examples/java8-webapp/demo-webapp/src/main/java/com/kudi/coder/demowebapp/config/{KudiCoderProperties,SdkConfig,WebMvcConfig}.java`
- `examples/java8-webapp/demo-webapp/src/main/resources/{application.yml,data.sql}`
- `examples/java8-webapp/demo-webapp/src/test/java/com/kudi/coder/demowebapp/web/{AuthDemoControllerTest,TaskControllerTest,PermissionDemoControllerTest,AuditDemoControllerTest,ConfigDemoControllerTest,NotificationDemoControllerTest,FileDemoControllerTest}.java`

**修改**：

- `sdk/java/pom.xml` — 追加 audit-service-sdk 模块
- `sdk/java/README.md` — 追加 audit-service-sdk 章节

## 10. 后续可演进（不在本 spec 范围）

- audit-service-sdk 发布到内网 Maven 仓库
- demo-webapp 加 Docker 镜像（基于 `sdk/java/Dockerfile.builder` 模式）
- token 自动刷新拦截器（替代手动 refresh）
- 配置中心改 push / long-poll
- audit 上报改异步队列
- demo 集成 e2e（`tests/e2e` 加 java8-webapi 跑通套件）
