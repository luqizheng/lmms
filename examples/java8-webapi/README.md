# examples/java8-webapi

Spring Boot 2.1.18.RELEASE + Java 8 集成示例，演示对接 `coder-micro-services` 平台的 6 类能力 + 1 个业务（工单）示例。

> 6 类能力：**认证 / 权限 / 审计 / 配置 / 通知 / 文件**。
> 业务（工单）走 `@RequiresPermission` AOP，演示权限拦截与本地 H2 持久化。

## 跑通

### 1. 前置：起 Rust 服务栈

```bash
# 1) 基础设施（MySQL / Redis / Consul）
pnpm run dev:infra

# 2) 7 个 Rust 服务（api-gateway + iam + token + audit + config + file + notification）
pnpm run dev
```

外部 Maven 仓库（Reposilite `192.168.2.12:18181`）已在父 POM 配好，子模块不再重复声明。

### 2. 编译 + 启动 demo

```bash
# 1) 装 audit-service-sdk 到本地仓库（手动 SDK，不在公共仓库）
cd sdk/java
mvn -Pjava8 -pl audit-service-sdk -am install -DskipTests
cd -

# 2) 编译整个 examples 工程
cd examples/java8-webapi
mvn -Pjava8 clean install

# 3) 启动 demo（端口 7000）
mvn -Pjava8 -pl demo-webapp spring-boot:run
```

### 3. 跑演示

```bash
bash examples/java8-webapi/scripts/demo.sh
```

预期：15 个 curl 全部得到 JSON 响应，每步末尾打印 `ok` 标记。

> 可覆盖的环境变量：
> - `BASE_URL` — demo-webapp 地址，默认 `http://localhost:7000`
> - `USER_ID` — `X-User-Id` 头，默认 `alice`
>
> 平台服务的 `kudi.gateway.base-url` 由 `KUDI_GATEWAY_URL` 注入，默认 `http://localhost:3000`。

## 6 类能力 ↔ 端点

| 能力 | 端点 | SDK / 实现 |
|---|---|---|
| 认证 | `GET  /demo/auth/jwks` / `GET /demo/auth/me` / `POST /demo/auth/refresh` / `POST /demo/auth/revoke` | `token-server-user-sdk` + `user-service-user-sdk` |
| 权限 | `GET  /demo/perm/public` / `GET /demo/perm/internal` / `GET /demo/perm/resource` | `user-service-user-sdk` + `user-service-client-sdk` + `token-server-user-sdk` |
| 审计 | `POST /demo/audit/event` / `POST /demo/audit/event/soft` / `GET /demo/audit/logs` | `audit-service-sdk`（手写；软失败走 `AuditPublisher`） |
| 配置 | `GET  /demo/config/show?key=...` / `GET /demo/config/wait-change?key=...&expected=...&timeoutSec=...` | `kudi-http-client` + `ConfigPoller`（5s 轮询） |
| 通知 | `POST /demo/notify/task-done` | `kudi-http-client` |
| 文件 | `POST /demo/file/upload`（multipart）/ `GET /demo/file/{id}` | `kudi-http-client` |
| 业务（工单） | `/demo/tasks` / `/demo/tasks/{id}` / `/demo/tasks/{id}/assign` / `/demo/tasks/{id}/status` / `DELETE /demo/tasks/{id}` | `TaskService`（本地 H2 + `JwtAuthInterceptor` + `@RequiresPermission` AOP） |

> 端点全部以 `ApiResponse` 包装（`{code, message, data, request_id}`），异常走 `GlobalExceptionHandler`。
> 不在白名单的端点需带 `X-User-Id` 头；权限拦截的端点需带 `X-Permissions: <csv>`。
> 白名单：`/demo/auth/jwks`、`/demo/auth/me`、`/demo/perm/public`、`/error`。

## 测试

```bash
# SDK 单元测试（含 audit-service-sdk 4 个）
cd sdk/java
mvn -Pjava8 test

# Demo controller 烟雾测试（MockMvc，7 个 controller × N 用例 = 20 用例）
cd examples/java8-webapi
mvn -Pjava8 -pl demo-webapp test
```

MockMvc 测试用 `@Import` 真实 `JwtAuthInterceptor` + `WebMvcConfig` + `PermissionAspect`，
通过 `X-User-Id` / `X-Permissions` 头模拟网关注入的身份与权限。

## YAGNI

不做：Docker 镜像、Swagger UI、OTel、多租户、自动 token 刷新、CI 集成、异步队列、二进制流式下载、multipart 透传。
