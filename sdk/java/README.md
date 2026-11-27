# LMMS Java SDK

LMMS Java SDK 是一个 Maven 多模块项目，支持 Java 8 / 17 / 21。Maven Central 发布坐标使用 `io.github.luqizheng`；Java 源码包名仍保留历史 `io.digua.*`，本次只迁移 Maven artifact coordinates，不重命名 Java packages。

## 模块

| Module | Maven coordinate（groupId:artifactId） |
|---|---|
| common | `io.github.luqizheng:digua-sdk-common` |
| digua-user-user-sdk | `io.github.luqizheng:digua-user-user-sdk` |
| digua-user-client-sdk | `io.github.luqizheng:digua-user-client-sdk` |
| digua-token-user-sdk | `io.github.luqizheng:digua-token-user-sdk` |
| digua-token-client-sdk | `io.github.luqizheng:digua-token-client-sdk` |
| digua-http-client | `io.github.luqizheng:digua-http-client` |
| digua-notification-sdk | `io.github.luqizheng:digua-notification-sdk` |

父 POM：`io.github.luqizheng:digua-sdk-parent:1.1.2`。

## 编译与测试

需要 JDK 8 / 17 / 21 和 Maven 3.9+。

```bash
mvn -B -f sdk/java/pom.xml -Pjava8 clean verify
mvn -B -f sdk/java/pom.xml -Pjava17 clean verify
mvn -B -f sdk/java/pom.xml -Pjava21 clean verify
```

## Maven Central 发布

- 仅推送严格符合 `vMAJOR.MINOR.PATCH` 的 Git tag 才会触发上传。
- PR 和普通 branch push 只验证，不上传。
- GitHub Secrets：`MVN_USERNAME`、`MVN_PASSWORD`、`GPG_PRIVATE_KEY`、`GPG_PASSPHRASE`。
- Central Publishing 插件配置 `autoPublish=false`；上传并通过验证后，维护者必须在 Sonatype Central Deployments 页面人工点击 Publish。
- 本仓库不会自动创建 tag，也不会自动发布 Maven Central deployment。

详细配置见 [Java SDK 发布指南](../../docs-site/guides/java-sdk-publishing.md)。

## 引入 SDK

```xml
<dependency>
    <groupId>io.github.luqizheng</groupId>
    <artifactId>digua-token-client-sdk</artifactId>
    <version>1.1.2</version>
</dependency>
```

Java 源码 package 保持 `io.digua.*`。内网 Reposilite 发布入口仍保留，Central 发布通过独立 Maven profile 启用。
