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

仓库包含 `examples/java8-webapi/`，是 Spring Boot 2.1.18 + Java 8 的 Maven 多模块示例，包含 `demo-webapp`、`demo-shared` 和 `scripts/demo.sh`。

**当前迁移状态：** 原示例使用旧 SDK 坐标、顶层 Java imports 和已删除的 Audit SDK。针对这些问题，已在 [PR #26](https://github.com/luqizheng/lmms/pull/26) 提交迁移，PR 尚未合并；因此在 PR #26 合并前，`main` 分支中的示例仍应视为旧版示例，不可作为当前 SDK 的兼容性验收依据。

PR #26 的迁移内容包括：

- Maven 依赖统一到 `io.github.luqizheng`，SDK artifact version 使用 `1.1.2`；Java 源码包名继续使用 `io.digua.*`。
- 示例接入当前 SDK 提供的 IAM user/client、Token user/client、通用 HTTP 和 Notification 模块。
- 删除当前 SDK 不提供的旧 Audit SDK、手写 Config Poller、File 接口及与 SDK 验收无关的本地工单示例。
- 增加 Java 8 下游集成工作流：从源码构建并安装 SDK，再构建和测试 Spring Boot 示例。
- 增加 Spring Boot 应用启动及 MockMvc 冒烟测试；真实 Gateway 联调仍需可访问的服务栈和有效凭据。

本地验证命令（在 PR #26 分支或合并后的 main 上执行）：

```bash
mvn -B -f sdk/java/pom.xml -Pjava8 clean install
mvn -B -f examples/java8-webapi/pom.xml -Pjava8 clean verify
```

真实服务联调：

```bash
mvn -f examples/java8-webapi/pom.xml -Pjava8 -pl demo-webapp spring-boot:run
bash examples/java8-webapi/scripts/demo.sh
```

注意：MockMvc 冒烟测试不证明 Gateway 的真实 API、凭据或权限配置正确；必须单独运行 live smoke script 才能确认部署环境联通。本次代码迁移尚未在用户本机或真实 LMMS 服务环境执行 Maven 构建和 Gateway 联调。

## 10.6 与 Rust 的差异

- Rust 是本仓库优先维护的 SDK 路径。
- Java SDK 的模块覆盖面较窄；没有 Java file-service SDK。
- Java 历史模块曾有 12 处端点漂移，相关旧模块已删除，notification 路径已修正。
- Java SDK 通用 HTTP 客户端和服务端身份模型不能据此推断为支持所有 Rust 内部服务调用。

## 10.7 发布坐标与发布安全

公共发布使用 Maven Central Namespace `io.github.luqizheng`。仅严格符合 `vMAJOR.MINOR.PATCH` 的 Git tag 触发上传；PR/branch push 只验证。Central 上传后 `autoPublish=false`，维护者需在 Sonatype Central 手动 Publish。不要为了消费 SDK 而自行创建 tag 或触发发布。

详细发布说明见 [Java SDK README](../../sdk/java/README.md) 和 [Java SDK 发布指南](../guides/java-sdk-publishing.md)。
