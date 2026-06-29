# KudiCoder Java SDK

`iam-service` / `token-service` 的 Java 8 / 17 / 21 客户端 SDK。

## 模块

| Module | groupId : artifactId | 说明 |
|---|---|---|
| `common` | `com.kudi.coder:kudi-coder-sdk-common` | `HttpClient` 包装、`TokenCache`、模型、`SdkException` 体系 |
| `user-service-user-sdk` | `com.kudi.coder:user-service-user-sdk` | 终端用户 → `iam-service`（`username + password`） |
| `user-service-client-sdk` | `com.kudi.coder:user-service-client-sdk` | 服务间 → `iam-service`（`api_key + secret`） |
| `token-server-user-sdk` | `com.kudi.coder:token-server-user-sdk` | 终端用户登录 + `token-service`（刷新 / 撤销 / 资源令牌） |
| `token-server-client-sdk` | `com.kudi.coder:token-server-client-sdk` | 服务间 `client_credentials` 调用 `token-service` |
| `kudi-http-client` | `com.kudi.coder:kudi-http-client` | 轻量 HTTP 客户端，JWT 透传模型，适合无专属 SDK 的下游服务 |
| `audit-service-sdk` | `com.kudi.coder:audit-service-sdk` | 业务侧 → `audit-service`（上报事件、查日志、列事件类型） |

> artifact ID 沿用历史命名（`user-service` / `token-server`），但实际打通的服务已经是合并后的 `iam-service` 和 `token-service`。后续大版本再考虑 artifact 重命名。

## 依赖

父 POM：`com.kudi.coder:kudi-coder-sdk-parent:0.2.0`，统一 `groupId` / `version` / 插件。

运行时：

- `com.fasterxml.jackson.core:jackson-databind:2.16.1`
- `com.fasterxml.jackson.datatype:jackson-datatype-jsr310:2.16.1`

测试：

- `org.junit.jupiter:junit-jupiter:5.10.1`（test）
- `com.squareup.okhttp3:mockwebserver:4.12.0`（test）

## 编译

### 方式 A：本地 Maven（需要本机有 JDK + mvn）

`source` / `target` 通过 Maven profile 切换：

```bash
mvn -Pjava8  clean package     # 默认 profile
mvn -Pjava17 clean package
mvn -Pjava21 clean package
```

### 方式 B：Docker 容器内编译（推荐，本机零依赖）

行为对齐 Rust 服务的 `scripts/run-tests-docker.js` —— 源码 + `~/.m2` 缓存通过 volume mount，不打项目镜像、不烤源码：

```bash
# 默认 verify（编译 + 测试 + 打包）
node scripts/build-java-sdk-docker.js

# 单独动作
node scripts/build-java-sdk-docker.js test        # 只跑测试
node scripts/build-java-sdk-docker.js compile     # 只编译
node scripts/build-java-sdk-docker.js package     # 编译 + 打包（跳过测试）
node scripts/build-java-sdk-docker.js deploy      # 发到内网 Maven 仓库

# 切 profile
node scripts/build-java-sdk-docker.js test --profile java17
node scripts/build-java-sdk-docker.js test --profile java21

# 透传额外 mvn 参数（-- 之后的内容会原样传给 mvn）
node scripts/build-java-sdk-docker.js test -- -pl user-service-user-sdk -am
```

要点：

- Builder 镜像：`zhcoder-docker-registry.com:8000/builder/java-sdk:0.2.0`（定义在 `sdk/java/Dockerfile.builder`）；脚本检测本地不存在时会自动 `docker build` 一次（约 60s），后续 0 build。
- Maven 缓存（`~/.m2`）通过命名 volume `coder-java-m2` 持久化，跨次复用。
- 镜像默认 JDK 21，pom 的 `-Pjava8 / -Pjava17 / -Pjava21` 只切换 `source/target` 字节码版本，JDK 21 可向下编译到 1.8。如果需要在不同 JVM **运行时**矩阵测试，那是 CI 的事，本镜像不覆盖。

## 引入

```xml
<dependency>
  <groupId>com.kudi.coder</groupId>
  <artifactId>user-service-user-sdk</artifactId>
  <version>0.2.0</version>
</dependency>
```

## 网络

所有调用统一走 `api-gateway` 的单一 `baseUrl`（例：`https://gw.kudi.repos`）。SDK 内部使用 v1 路由（`/v1/iam/...`、`/v1/tokens/...`、`/v1/api-keys`）。

## 0.2.0 Changelog

破坏性变更，从 0.1.0 升级需关注：

- 路径迁移到 v1：登录 `/api/v1/auth/login` → `/v1/iam/auth/login`；用户/角色/权限 `/api/...` → `/v1/iam/...`；token 端点 `/api/tokens/...` → `/v1/tokens/...`；`/api/api-keys` → `/v1/api-keys`。
- 所有请求体改为 **JSON**（`application/json`），不再使用 `application/x-www-form-urlencoded`。
- `TokenServerUserClient` / `TokenServerClientClient` 的 `revoke(...)` 签名改为 `revoke(String tokenJti, long expiresIn)`。
- `issueResource(...)` 签名改为 `issueResource(String resourceType, String resourceId, String permission, Long ttlSec)`，返回类型改为新的 `ResourceTokenInfo`（字段 `resource_token` / `expires_in`）。
- `TokenServerClientClient.createApiKey(...)` 签名改为 `createApiKey(String name, List<String> permissions, List<String> grantTypes)`；返回的 `ApiKeyInfo` 字段重命名为 `apiKeyId` / `apiKeySecret`，去掉 `createdAt`。
- `kudi-http-client` 模块在 0.2.0 重写为 **纯 JWT 透传 HTTP 客户端**：移除原本的 `client_credentials` token 获取 / 缓存（与本平台 `authorization_code`-only 的 `/tokens/access` 不兼容），主类 `KudiWebClient` 重命名为 `KudiHttpClient`，API 简化为 `get` / `post` / `put` / `delete`（均接收一个可选 `bearer` 参数）。需要换 token 的场景请改用 `user-service-user-sdk` 或 `token-server-client-sdk`。

## 仓库

- Release：`http://mvn.kudi.repos/repository/maven-releases/`（Reposilite，id=`kudi-maven`）
- Snapshot：`http://mvn.kudi.repos/repository/maven-snapshots/`

## 发布

CI 触发 `v*.*.*` tag，本地不入库：

```bash
mvn -Pjava8 -DskipTests deploy
```

详细 spec 见 `docs/superpowers/specs/2026-06-08-multi-lang-sdk-design.md`。
