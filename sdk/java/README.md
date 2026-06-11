# KudiCoder Java SDK

`user-service` / `token-server` 的 Java 8 / 17 / 21 客户端 SDK。

## 模块

| Module | groupId : artifactId | 说明 |
|---|---|---|
| `common` | `com.kudi.coder:kudi-coder-sdk-common` | `HttpClient` 包装、`TokenCache`、模型、`SdkException` 体系 |
| `user-service-user-sdk` | `com.kudi.coder:user-service-user-sdk` | 终端用户 → `user-service`（`username + password`） |
| `user-service-client-sdk` | `com.kudi.coder:user-service-client-sdk` | 服务间 → `user-service`（`api_key + secret`） |
| `token-server-user-sdk` | `com.kudi.coder:token-server-user-sdk` | 终端用户登录 / 刷新 / 登出 |
| `token-server-client-sdk` | `com.kudi.coder:token-server-client-sdk` | 服务间 `client_credentials` |

## 依赖

父 POM：`com.kudi.coder:kudi-coder-sdk-parent:0.1.0`，统一 `groupId` / `version` / 插件。

运行时：

- `com.fasterxml.jackson.core:jackson-databind:2.16.1`
- `com.fasterxml.jackson.datatype:jackson-datatype-jsr310:2.16.1`

测试：

- `org.junit.jupiter:junit-jupiter:5.10.1`（test）
- `com.squareup.okhttp3:mockwebserver:4.12.0`（test）

## 编译

`source` / `target` 通过 Maven profile 切换：

```bash
mvn -Pjava8  clean package     # 默认 profile
mvn -Pjava17 clean package
mvn -Pjava21 clean package
```

## 引入

```xml
<dependency>
  <groupId>com.kudi.coder</groupId>
  <artifactId>user-service-user-sdk</artifactId>
  <version>0.1.0</version>
</dependency>
```

## 网络

所有调用统一走 `api-gateway` 的单一 `baseUrl`（例：`https://gw.kudi.repos`）。

## 仓库

- Release：`http://mvn.kudi.repos/repository/maven-releases/`（Reposilite，id=`kudi-maven`）
- Snapshot：`http://mvn.kudi.repos/repository/maven-snapshots/`

## 发布

CI 触发 `v*.*.*` tag，本地不入库：

```bash
mvn -Pjava8 -DskipTests deploy
```

详细 spec 见 `docs/superpowers/specs/2026-06-08-multi-lang-sdk-design.md`。
