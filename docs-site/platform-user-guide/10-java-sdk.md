# 10. Java SDK（次要接入方式）

## 10.1 定位

Java SDK 位于 `sdk/java/`，是 Maven 多模块工程，支持 Java 8 / 17 / 21。当前公共 Maven 坐标使用：

```xml
<dependency>
  <groupId>io.github.luqizheng</groupId>
  <artifactId>digua-token-client-sdk</artifactId>
  <version><!-- 使用已发布版本 --></version>
</dependency>
```

Java 源码 package 仍保留 `io.digua.*`。不要将历史文档中的 `io.digua` groupId 当作当前 Maven Central 坐标。

## 10.2 模块

| Module | Artifact ID | 作用 |
|---|---|---|
| common | `digua-sdk-common` | 公共模型、配置和异常 |
| digua-user-user-sdk | `digua-user-user-sdk` | 用户身份视角的 IAM 调用 |
| digua-user-client-sdk | `digua-user-client-sdk` | client credentials 视角的 IAM 调用 |
| digua-token-user-sdk | `digua-token-user-sdk` | 用户身份视角的 Token 调用 |
| digua-token-client-sdk | `digua-token-client-sdk` | 客户端身份视角的 Token 调用 |
| digua-http-client | `digua-http-client` | 通用 HTTP 客户端 |
| digua-notification-sdk | `digua-notification-sdk` | 通知/公告 API |

Java SDK 当前不提供与 Rust SDK 完全相同的所有服务模块。历史 audit/config/file SDK 模块已因 API 漂移删除；不要使用旧 artifact 名称。

## 10.3 引入和构建

```xml
<dependency>
  <groupId>io.github.luqizheng</groupId>
  <artifactId>digua-user-user-sdk</artifactId>
  <version><!-- 当前发布版本 --></version>
</dependency>
```

在仓库内验证：

```bash
mvn -B -f sdk/java/pom.xml -Pjava8 clean verify
mvn -B -f sdk/java/pom.xml -Pjava17 clean verify
mvn -B -f sdk/java/pom.xml -Pjava21 clean verify
```

## 10.4 使用示意

用户身份 IAM client 的 README 示例：

```java
UserServiceUserClient client = UserServiceUserClient.login(
    "https://gw.example.com", "alice", "<password>");
User user = client.getById("user-id");
```

client credentials 形式的管理 SDK 示例：

```java
UserServiceClientClient client = UserServiceClientClient.create(
    "https://gw.example.com", "<client-id-or-api-key>",
    "<client-secret>", "<authorization-code>");
```

上述类名和参数来自各模块 README 的示意；具体构造方式、凭证类型、grant 支持范围和 API 方法必须以所用 SDK 版本源码/Javadoc 为准。不要把 client credentials secret 放进客户端应用。

## 10.5 与 Rust 的差异

- Rust 是本仓库优先维护的 SDK 路径。
- Java SDK 的模块覆盖面较窄；没有 Java file-service SDK。
- Java 历史模块曾有 12 处端点漂移，相关旧模块已删除，notification 路径已修正。
- Java SDK 通用 HTTP 客户端和服务端身份模型不能据此推断为支持所有 Rust 内部服务调用。

## 10.6 发布坐标与发布安全

公共发布使用 Maven Central Namespace `io.github.luqizheng`。仅严格符合 `vMAJOR.MINOR.PATCH` 的 Git tag 触发上传；PR/branch push 只验证。Central 上传后 `autoPublish=false`，维护者需在 Sonatype Central 手动 Publish。不要为了消费 SDK 而自行创建 tag 或触发发布。

详细发布说明见 [Java SDK README](../../sdk/java/README.md) 和 [Java SDK 发布指南](../guides/java-sdk-publishing.md)。
