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

## 10.5 仓库内 Java Web 示例与集成测试

仓库包含 `examples/java8-webapi/`，是一个 Spring Boot 2.1.18 + Java 8 的 Maven 多模块示例，包含 `demo-webapp`、`demo-shared`、MockMvc 测试和 `scripts/demo.sh`。入口见 [examples/java8-webapi README](../../examples/java8-webapi/README.md)。

**重要：当前示例仍是旧版 SDK 集成，不能直接作为当前 `sdk/java/` 的兼容性验收。** 静态核对发现：

- 示例父 POM 的依赖坐标仍使用 `io.digua`，并将多个 SDK 固定为 `0.2.0`；当前 SDK 源码模块使用 `io.github.luqizheng` Maven 坐标，父 POM 版本为 `1.1.2`。
- 示例源码仍从 `io.digua.*` 顶层导入旧客户端类；当前源码类位于 `io.digua.user`、`io.digua.token`、`io.digua.httpclient` 等 package。
- 示例依赖 `audit-service-sdk`，但该模块不在当前 `sdk/java/pom.xml` 的模块列表中；当前 Java SDK 也没有独立的 Audit、Config、File SDK 模块。
- 示例 README 中的 `pnpm run dev:infra` / `pnpm run dev` 与当前根 `package.json` 脚本不一致。当前脚本使用 `pnpm run dev:infra:pg`、`pnpm run dev:pg`（或对应 MySQL 脚本）。

因此，示例现有 MockMvc 测试只能说明示例自身的 Spring MVC 逻辑，不证明当前 Java SDK 的依赖、API 或运行时兼容性。不要将示例 README 中“15 个 curl 全部成功”的预期当成已验证的当前版本结果。

当前建议的验证顺序：

1. 先运行当前 Java SDK 自身测试：`mvn -B -f sdk/java/pom.xml -Pjava8 clean verify`。
2. 若需使用 `examples/java8-webapi` 做下游集成验收，先将其依赖坐标、版本、package imports 和被删除的 SDK 能力迁移到当前模块范围，再执行示例构建、MockMvc 测试和 Gateway 联调。
3. 联调时明确记录平台服务端版本与 Maven SDK artifact 版本；两者是独立版本号，不能仅凭平台发布版本推断 Java SDK artifact 版本。

本次文档审查只完成了仓库静态核对；未在开发机或内部 Maven 仓库执行 Maven 构建，也未对正在运行的 LMMS 环境发起联调。

## 10.6 与 Rust 的差异

- Rust 是本仓库优先维护的 SDK 路径。
- Java SDK 的模块覆盖面较窄；没有 Java file-service SDK。
- Java 历史模块曾有 12 处端点漂移，相关旧模块已删除，notification 路径已修正。
- Java SDK 通用 HTTP 客户端和服务端身份模型不能据此推断为支持所有 Rust 内部服务调用。

## 10.7 发布坐标与发布安全

公共发布使用 Maven Central Namespace `io.github.luqizheng`。仅严格符合 `vMAJOR.MINOR.PATCH` 的 Git tag 触发上传；PR/branch push 只验证。Central 上传后 `autoPublish=false`，维护者需在 Sonatype Central 手动 Publish。不要为了消费 SDK 而自行创建 tag 或触发发布。

详细发布说明见 [Java SDK README](../../sdk/java/README.md) 和 [Java SDK 发布指南](../guides/java-sdk-publishing.md)。
