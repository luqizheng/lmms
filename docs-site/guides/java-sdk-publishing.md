# Java SDK 发布指南

当前公共发布目标为 Sonatype Central Portal。内部 Reposilite 发布入口仍保留，二者通过独立 Maven profile 隔离。

## 1. 模块与 Maven 坐标

父 POM：`io.github.luqizheng:digua-sdk-parent:1.1.1`。

| Module | Artifact ID |
|---|---|
| common | digua-sdk-common |
| digua-user-user-sdk | digua-user-user-sdk |
| digua-user-client-sdk | digua-user-client-sdk |
| digua-token-user-sdk | digua-token-user-sdk |
| digua-token-client-sdk | digua-token-client-sdk |
| digua-http-client | digua-http-client |
| digua-notification-sdk | digua-notification-sdk |

所有发布坐标的 groupId 均为 `io.github.luqizheng`。Java 源码 package 仍使用历史 `io.digua.*`，不要求重命名。

## 2. Sonatype Central 一次性准备

1. 登录 [Sonatype Central](https://central.sonatype.com/)。
2. 在 Publishing Settings → Namespaces 确认 `io.github.luqizheng` 已验证。使用 GitHub 账号 `luqizheng` 登录时，Sonatype 通常会自动配置该个人 Namespace；仍以 Portal 显示为准。
3. 在 [User Token](https://central.sonatype.com/usertoken) 生成 User Token。GitHub Secrets `MVN_USERNAME` 和 `MVN_PASSWORD` 应分别保存 Token Username 和 Token Password，不是网站登录密码。
4. 生成 GPG 密钥并把公钥上传到公钥服务器。
5. 在 GitHub 仓库 Settings → Secrets and variables → Actions 添加：
   - `MVN_USERNAME`：Sonatype User Token Username（已有）
   - `MVN_PASSWORD`：Sonatype User Token Password（已有）
   - `GPG_PRIVATE_KEY`：ASCII-armored secret key
   - `GPG_PASSPHRASE`：该私钥的密码

### 2.1 生成 GPG 密钥（Linux）

```bash
sudo apt-get update
sudo apt-get install -y gnupg
gpg --full-generate-key
```

建议选择 RSA and RSA、4096 bit，并设置强密码。查看密钥 ID：

```bash
gpg --list-secret-keys --keyid-format LONG
```

发布公钥（将 KEY_ID 替换为实际值）：

```bash
gpg --keyserver keyserver.ubuntu.com --send-keys KEY_ID
```

导出私钥：

```bash
gpg --armor --export-secret-keys KEY_ID
```

将完整输出（包括 BEGIN/END 行）保存为 GitHub Secret `GPG_PRIVATE_KEY`；密钥密码保存为 `GPG_PASSPHRASE`。私钥不得提交到 Git、构建产物或日志。建议同时离线备份私钥。

## 3. 本地验证

需要 JDK 21 和 Maven 3.9+：

```bash
mvn -B -f sdk/java/pom.xml -Pjava8 clean verify
mvn -B -f sdk/java/pom.xml -Pjava17 clean verify
mvn -B -f sdk/java/pom.xml -Pjava21 clean verify
```

父 POM 统一生成 Sources JAR 和 Javadoc JAR。普通 verify 不激活 GPG，也不会访问 Central。

## 4. 正式上传规则（Tag-only）

专用工作流：`.github/workflows/java-sdk-maven-central.yml`。

| 事件 | 行为 |
|---|---|
| Pull Request 到 main | Java 8 / 17 / 21 构建、测试、Sources/Javadoc 验证；不读取发布凭据、不上传 |
| 普通 branch push | 不触发该工作流 |
| 非 semver tag | 不匹配发布触发器 |
| `vMAJOR.MINOR.PATCH` tag | 校验 tag 与父 POM version 一致，签名并上传 Central Portal |

正式上传前必须先合并 PR、准备四个 GitHub Secrets，再由维护者创建并推送与 POM 版本完全一致的 tag。工作流不提供手动 dispatch，不自动创建 tag。

上传命令的等价形式：

```bash
mvn -B -f sdk/java/pom.xml -Pjava8,central-publishing,central-release clean deploy
```

`central-publishing` profile 使用 Central Publishing Maven Plugin；`central-release` profile 执行 GPG 签名。两者均不在普通构建中默认启用。

## 5. 人工 Publish

Central 插件配置 `autoPublish=false`。GitHub Actions 成功表示 bundle 已上传并完成 Portal 验证等待，不代表构件已经公开发布。

维护者必须：

1. 打开 [Sonatype Central Deployments](https://central.sonatype.com/publishing/deployments)。
2. 找到本次 tag 对应的 Deployment。
3. 检查所有 validation results。
4. 确认 Namespace、POM 元数据、JAR、Sources、Javadoc、GPG signatures 均正确。
5. 人工点击 **Publish**。

Maven Central release 一经发布不可覆盖同一坐标版本；修复必须提升版本号并重新发布。

## 6. 消费 SDK

```xml
<dependency>
    <groupId>io.github.luqizheng</groupId>
    <artifactId>digua-token-client-sdk</artifactId>
    <version>1.1.1</version>
</dependency>
```

其余模块替换 artifactId 即可。Central 同步完成后，消费者不需要配置私有仓库。

## 7. 内部 Reposilite 发布

原有内部发布 CLI（默认 curl 上传）保留。为避免私有地址进入公开发布的 POM，Reposilite 的 `distributionManagement` 已移动到非默认的 `reposilite-publishing` profile。若需要显式使用 `mvn deploy` 上传内部仓库，执行 `-Preposilite-publishing`。Central 上传使用独立的 `central-publishing` profile；两者不可同时启用。内部 Reposilite 凭证与 Central User Token 不应混用。
