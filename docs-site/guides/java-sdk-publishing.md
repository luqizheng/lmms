# Java SDK 发布指南

> 适用：把 `sdk/java/` 下的 Maven 多模块发布到内网私有 Maven 仓库（Reposilite）。
> 关联：[`sdk/java/README.md`](../../sdk/java/README.md)（SDK 用法）、[`scripts/publish-maven-release.sh`](../../scripts/publish-maven-release.sh)（主发布脚本）、[`scripts/publish-sdks.sh`](../../scripts/publish-sdks.sh)（旧 4 模块脚本）。
> 维护：本文档随发布脚本和 Reposilite 政策变化同步更新。

---

## 1. SDK Maven 模块总览

```
sdk/java/
├── pom.xml                                    # kudi-coder-sdk-parent（聚合 pom）
├── common                                     # kudi-coder-sdk-common（基础）
├── user-service-user-sdk                      # 用户身份 → iam-service
├── user-service-client-sdk                    # 客户端身份 → iam-service
├── token-server-user-sdk                      # 用户身份 → token-service
├── token-server-client-sdk                    # 客户端身份 → token-service
├── kudi-http-client                           # 通用服务间 HTTP 客户端
├── audit-service-sdk
├── config-service-sdk
├── file-service-sdk
└── notification-service-sdk
```

| Maven artifactId | groupId | 用途 |
| --- | --- | --- |
| `kudi-coder-sdk-common` | `com.kudi.coder` | 公共配置/异常（其余 SDK 自动传递依赖） |
| `kudi-http-client` | `com.kudi.coder` | 底层 HTTP 客户端封装 |
| `user-service-user-sdk` | `com.kudi.coder` | 用户身份视角调用 IAM |
| `user-service-client-sdk` | `com.kudi.coder` | 客户端应用视角调用 IAM |
| `token-server-user-sdk` | `com.kudi.coder` | 用户身份视角调用 Token |
| `token-server-client-sdk` | `com.kudi.coder` | 客户端应用视角调用 Token |
| `audit-service-sdk` | `com.kudi.coder` | 调用审计服务 |
| `config-service-sdk` | `com.kudi.coder` | 调用配置中心 |
| `file-service-sdk` | `com.kudi.coder` | 调用文件服务 |
| `notification-service-sdk` | `com.kudi.coder` | 调用通知服务 |

> **目标仓库**：Reposilite `http://192.168.2.12:18181/`
> - release：http://192.168.2.12:18181/releases（`<version>` 不能带 `-SNAPSHOT`）
> - snapshot：http://192.168.2.12:18181/snapshots（`<version>` 必须带 `-SNAPSHOT`）

---

## 2. 前置条件

| # | 条件 | 怎么验证 |
| - | --- | --- |
| 1 | JDK 8 / 17 / 21 任一（默认 profile `java8`），**或用 Docker 容器模式** | `java -version`、`mvn -version`，或 `docker --version` |
| 2 | Maven 3.9+，**或用 Docker 容器模式** | `mvn -version` |
| 3 | 网络可达 `192.168.2.12:18181` | `curl -sI http://192.168.2.12:18181/` |
| 4 | `~/.m2/settings.xml` 配置了仓库凭证（见 §3.1） | `grep reposilite ~/.m2/settings.xml` |

> 💡 **没有 JDK / Maven 想发布？** 推荐用 `node scripts/build-java-sdk-docker.js deploy`（基于 `zhcoder-docker-registry.com:8000/builder/java-sdk:0.2.0` 镜像，脚本自动构建）。不需要本机安装任何 Java 工具链。

---

## 3. 一次性配置

### 3.1 凭证（两种方式二选一）

#### 方式 A：写入 `~/.m2/settings.xml`（推荐用于 CI）

```xml
<!-- ~/.m2/settings.xml -->
<settings xmlns="http://maven.apache.org/SETTINGS/1.0.0">
  <servers>
    <server>
      <id>reposilite-repository-releases</id>
      <username>your-username</username>
      <password>your-password</password>
    </server>
    <server>
      <id>reposilite-repository-snapshots</id>
      <username>your-username</username>
      <password>your-password</password>
    </server>
  </servers>
</settings>
```

⚠️ `~/.m2/settings.xml` 是默认配置，**不要 commit 到 git**。把 `settings.xml` 加入 `.gitignore`（多数项目已默认）。

#### 方式 B：环境变量 / 命令行（CI 友好）

```bash
export MAVEN_USERNAME=admin
export MAVEN_PASSWORD='s3cret'

bash scripts/publish-maven-release.sh        # release 版本
bash scripts/publish-maven-snapshot.sh      # snapshot 版本
```

或者命令行直接传：

```bash
bash scripts/publish-maven-release.sh --user admin --pass 's3cret'
```

### 3.2 server id 与 pom 中的 `<distributionManagement>` 对齐

`sdk/java/pom.xml` 的 `<distributionManagement>` 声明了 server id。当前（v0.3.0+）使用直接匹配 `settings.xml` 硬编码凭证的 ID：

```xml
<distributionManagement>
  <repository>
    <id>reposilite-repository-releases</id>
    <url>http://192.168.2.12:18181/releases</url>
  </repository>
  <snapshotRepository>
    <id>reposilite-repository-snapshots</id>
    <url>http://192.168.2.12:18181/snapshots</url>
  </snapshotRepository>
</distributionManagement>
```

如果你用 `publish-maven-release.sh`（`curl` 模式），脚本会通过 `-DaltDeploymentRepository` 覆盖 deploy 目标，**不需要改 pom**。

> ⚠️ **历史背景**：v0.2.0 及之前用 `id=kudi-maven`（依赖环境变量 `MAVEN_USER` / `MAVEN_PASS`）。v0.3.0 起改为 `reposilite-repository-releases` / `reposilite-repository-snapshots`，直接匹配 `settings.xml` 中硬编码的凭证，避免环境变量传递问题。

### 3.3 镜像（可选，加速第三方依赖下载）

如果用 aliyun / 内部 mirror 加速 maven central，可以在 `~/.m2/settings.xml` 加：

```xml
<mirrors>
  <mirror>
    <id>aliyun-public</id>
    <mirrorOf>central</mirrorOf>
    <name>Aliyun Public</name>
    <url>https://maven.aliyun.com/repository/public</url>
  </mirror>
</mirrors>
```

---

## 4. 发布顺序与依赖关系

```
                        kudi-coder-sdk-parent  (pom packaging, 不发布)
                                     │
        ┌────────────────────────────┼────────────────────────────┐
        ▼                            ▼                            ▼
kudi-coder-sdk-common        kudi-http-client          (user/token-server SDK — 互不依赖，但都依赖 common)
        │                                                      ▲
        ├────────────────────┬────────────────────┬────────────┤
        ▼                    ▼                    ▼            │
user-service-user-sdk user-service-client-sdk  ...   token-server-user-sdk
        ▲                                                  ▲
        └─── audit/config/file/notification-service-sdk ───┘
                  (依赖 common，不依赖 user/token-server SDK)
```

发布顺序：

1. 先发 `kudi-coder-sdk-common` 和 `kudi-http-client`（基础库，service SDK 都依赖）
2. 再发其余 9 个 service SDK（之间无依赖关系）

由于 maven 在多模块构建时会按依赖拓扑自动排序，**直接 `mvn deploy -am` 即可**（`--also-make` 自动构建依赖项）。

---

## 5. 发布流程

### 5.1 用发布脚本（推荐）

`scripts/publish-maven-release.sh` 和 `scripts/publish-maven-snapshot.sh` 是统一入口，支持 release / snapshot 两种场景。

```bash
# Release 版本（不带 -SNAPSHOT）
bash scripts/publish-maven-release.sh

# Snapshot 版本（带 -SNAPSHOT）
bash scripts/publish-maven-snapshot.sh
```

**参数：**

| 参数 | 说明 |
| --- | --- |
| `--profile <ver>` | 编译 profile：`java8` / `java17` / `java21`，默认 `java8` |
| `--modules "a,b,c"` | 只发指定模块（Maven `-pl` 语法），默认全量 |
| `--user <name>` `--pass <pwd>` | Reposilite 凭证（覆盖 settings.xml） |
| `--repo-id <id>` | 自定义 server id，默认 `reposilite-repository-releases` |
| `--repo-url <url>` | 自定义仓库 URL，默认 `http://192.168.2.12:18181/releases` |
| `--docker` | 在 docker 容器内跑（无需本机 JDK/Maven） |
| `--mvn` | 强制使用 `mvn deploy`（默认是 `mvn install + curl`，绕过 Reposilite 的 Expect:100-continue bug） |
| `--dry-run` | 只 verify，不实际 deploy |
| `--` | 透传额外 mvn 参数：`bash scripts/... -- -pl xxx -am -DskipTests` |

**脚本做的事：**

1. **前置检查**：JDK/Maven/curl 在 PATH；版本号符合 release/snapshot 规则（release 不能有 -SNAPSHOT 后缀，snapshot 必须有）。
2. **解析版本**：从 `sdk/java/pom.xml` 读 `<version>`。
3. **构建依赖**：`mvn -P<profile> -DskipTests install` 把所有模块装到本地 `~/.m2/repository`。
4. **上传**（默认 `curl` 模式）：遍历 `~/.m2/repository/com/kudi/coder/`，逐个 `curl -T upload` 到 Reposilite。**绕过 Maven Aether 的 Expect:100-continue 与 Reposilite 不兼容的 bug**。
5. **汇总**：成功 / 失败的 artifact。

> ⚠️ **`curl` 模式不是 mvn deploy**：先 `mvn install` 到本地仓库，再用 `curl` 单独 PUT 到 Reposilite。这是为了规避 Reposilite 的已知问题（HTTP 400 on Expect:100-continue）。`--mvn` 模式才用真正的 `mvn deploy`。

### 5.2 用 Docker 容器发布（推荐，无需本机 JDK/Maven）

`scripts/build-java-sdk-docker.js` 支持在 Docker 容器内编译 / 测试 / 打包 / 发布 SDK，行为对齐 Rust 服务的 `scripts/run-tests-docker.js`。

```bash
# 默认 verify（编译 + 测试 + 打包）
node scripts/build-java-sdk-docker.js

# 发布到内网 Maven 仓库（Release）
node scripts/build-java-sdk-docker.js deploy

# 只测试
node scripts/build-java-sdk-docker.js test

# 指定 profile
node scripts/build-java-sdk-docker.js deploy --profile java21
```

**脚本做的事：**

1. 自动检测 builder 镜像（`zhcoder-docker-registry.com:8000/builder/java-sdk:0.2.0`），不存在则构建
2. 挂载源码到容器，`~/.m2` 通过命名 volume `coder-java-m2` 持久化缓存
3. 挂载宿主机 `~/.m2/settings.xml`（只读），透传 `MAVEN_USER` / `MAVEN_PASS` 环境变量
4. 在容器内执行 `mvn -Pjava8 deploy -DskipTests`

> 本方法与 `publish-maven-release.sh` 的 `--docker` 模式等价，但更轻量——直接 `node scripts/build-java-sdk-docker.js deploy` 一行搞定，不需要单独构建 builder 镜像。

### 5.3 典型发布循环

#### 第一次发 release

```bash
# 1. 确认 sdk/java/pom.xml 的 <version> 不带 -SNAPSHOT
grep -A 1 "<version>" sdk/java/pom.xml | head -3

# 2. dry-run 验证
bash scripts/publish-maven-release.sh --dry-run

# 3. 真发布（推荐方式 A：Docker 容器发布）
node scripts/build-java-sdk-docker.js deploy

# 或方式 B：本机有 JDK 时用发布脚本
bash scripts/publish-maven-release.sh

# 4. 验证：在临时项目里引用 SDK
```

#### Bump 版本号 → 再发 release

```bash
# 0.3.0 → 0.4.0
mvn -f sdk/java/pom.xml versions:set -DnewVersion=0.4.0 versions:commit
# 检查 diff，确认无误
git diff sdk/java/pom.xml
# commit
git add sdk/java && git commit -m "feat(sdk): bump to 0.4.0"

# 发布
bash scripts/publish-maven-release.sh
```

#### 发 snapshot

```bash
# 0.3.0 → 0.3.1-SNAPSHOT
mvn -f sdk/java/pom.xml versions:set -DnewVersion=0.3.1-SNAPSHOT versions:commit
git add sdk/java && git commit -m "feat(sdk): start 0.3.1-SNAPSHOT"

# 发布
bash scripts/publish-maven-snapshot.sh
```

### 5.4 旧脚本 Docker 模式

如果本机没装 JDK / Maven，且想用 `publish-maven-release.sh` 的 `curl` 模式：

```bash
# 先构建 builder 镜像（一次性）
docker build -f sdk/java/Dockerfile.builder -t kudi-maven-builder:latest sdk/java/

# 或直接拉取内网镜像
docker pull zhcoder-docker-registry.com:8000/official/maven:3.9-eclipse-temurin-17

# 发布（脚本会自动用 docker run -v ~/.m2 来跑 mvn）
bash scripts/publish-maven-release.sh --docker
```

Docker 模式下脚本会用 `docker run` 启动临时容器，挂载源码、`.m2/`、`~/.m2/settings.xml`，在容器内跑 mvn。

---

## 6. 发布后验证

```bash
# 1. 拉一个临时项目试装
mkdir /tmp/sdk-verify && cd /tmp/sdk-verify
mvn archetype:generate -DgroupId=com.test -DartifactId=verify -DarchetypeArtifactId=maven-archetype-quickstart -DinteractiveMode=false

# 2. 加依赖
cat > pom.xml << 'EOF'
<project>
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.test</groupId>
  <artifactId>verify</artifactId>
  <version>1.0</version>
  <packaging>jar</packaging>
  <dependencies>
    <dependency>
      <groupId>com.kudi.coder</groupId>
      <artifactId>token-server-client-sdk</artifactId>
      <version>0.3.0</version>
    </dependency>
  </dependencies>
</project>
EOF

# 3. 补仓库
mkdir -p ~/.m2 && cat > ~/.m2/settings.xml << 'EOF'
<settings>
  <profiles>
    <profile>
      <id>internal-repo</id>
      <repositories>
        <repository>
          <id>reposilite-repository-releases</id>
          <url>http://192.168.2.12:18181/releases</url>
        </repository>
      </repositories>
    </profile>
  </profiles>
  <activeProfiles>
    <activeProfile>internal-repo</activeProfile>
  </activeProfiles>
</settings>
EOF

# 4. 编译（应能拉到 token-server-client-sdk）
mvn compile
```

或在文档站浏览：[`./java-integration`](./java-integration) §「引入 SDK」给出 `pom.xml` 示例。

---

## 7. Reposilite 速率限制 / 容量

Reposilite 没有公开的速率限制文档，但有几个实战经验：

| 限制 | 阈值 | 触发后表现 |
| --- | --- | --- |
| 单次请求 body 大小 | 默认 50MB（容器里 `application.yml` 可调） | HTTP 413 Payload Too Large |
| 同时在线并发 | 看 Reposilite 内存配置 | HTTP 503 / 排队 |
| snapshot 保留策略 | 默认保留最近 N 个 / 7 天 | 老的 snapshot 会被清理 |

**应对：**

- **别并发发**：脚本默认串行发，不要自己用 `xargs -P` 并行。
- **大 artifact**：如果未来要发 100MB+ 的 jar，先联系 Reposilite 管理员调 `max-content-length`。
- **snapshot 失效**：开发期间依赖 `0.3.1-SNAPSHOT` 时，确认 Reposilite 上还能拉到；老的被清理了就 bump 到新的 snapshot。

---

## 8. 版本号管理

### 8.1 semver 规则

| 改动类型 | 版本号变化 |
| --- | --- |
| 修复 bug、不破坏 API | patch：`0.3.0 → 0.3.1` |
| 新增方法（向后兼容） | minor：`0.3.0 → 0.4.0` |
| 改动签名 / 删方法（破坏兼容） | major：`0.3.0 → 1.0.0` |

### 8.2 release vs snapshot

- **release**：稳定版，**不可重新发布**。同 version 第二次 deploy 报错 `409 Conflict`。如需修改，bump 到新版本号。
- **snapshot**：开发中版本，**可重复 deploy**。新 deploy 覆盖旧 artifact，时间戳戳记在 metadata 里。

### 8.3 一致性约束

- **聚合 pom + 所有子模块必须同版本号**。Maven 没有 path 依赖问题（`mvn -pl ... -am` 自动处理），但必须所有 `<parent>` 引用都对齐当前聚合版本。
- 用 `mvn versions:set -DnewVersion=<ver>` 一次性 bump 全部模块。

---

## 9. 故障排查

| 现象 | 原因 | 解决 |
| --- | --- | --- |
| `error: could not determine project version from sdk/java/pom.xml` | pom.xml 解析失败 | 检查 pom.xml 格式；mvn 是否能跑通 `help:evaluate` |
| `repository id is 'kudi-maven' but settings.xml only has 'reposilite-repository-releases'` | 老 pom.xml 与 settings.xml 的 server ID 不匹配 | v0.3.0+ 已改为 `reposilite-repository-releases`。如用旧版本，在 settings.xml 加 `<server><id>kudi-maven</id>` 或升级 pom.xml |
| `error: release version found in pom but script is publish-maven-snapshot.sh` | 跑错了脚本（snapshot 拒绝 release 版本） | 改用 `publish-maven-release.sh`，或先把版本改成 `-SNAPSHOT` |
| `error: pom is SNAPSHOT but script is publish-maven-release.sh` | 同上，反向 | 用 `publish-maven-snapshot.sh`，或去掉 `-SNAPSHOT` |
| `HTTP 400 Bad Request` on `mvn deploy` | 版本已存在，Reposilite 禁止重部署（`Redeployment is not allowed`） | **bump 版本号**再重试；或确认是否真的需要重部署同版本（release 原则上不可覆盖） |
| `HTTP 400 Bad Request` on `mvn deploy`（Expect:100-continue） | Reposilite 与 Maven Aether 的 Expect:100-continue 不兼容（旧版本 Reposilite） | 用 `build-java-sdk-docker.js deploy`（`mvn deploy` 直连）或 `publish-maven-release.sh`（`curl` 模式） |
| `407 Proxy Authentication Required` / `403 Forbidden` | Reposilite token 无效 / 没权限 | 找运维确认账号；或加 `--user` / `--pass` 重新传 |
| `404 Not Found` on upload | 仓库 URL 错 / snapshot 传错到 release | 检查 `--repo-url`；snapshot 脚本默认 `/snapshots`，release 默认 `/releases` |
| `mvn install` 成功但 `curl upload` 失败（HTTP 500） | Reposilite 内部错误 / 磁盘满 | 看 Reposilite 控制台；联系运维 |
| `Permission denied` 写 `~/.m2/repository` | 用户无权写本地 maven 仓库 | `chown -R $USER:$USER ~/.m2`；或在 docker 模式下用 `~/.m2` bind mount |
| 部分模块成功部分失败 | 中途网络中断 / Reposilite 抖动 | 重跑脚本（已成功的不会重复部署；release 部署失败可 bump 后重试） |
| 老的 `publish-sdks.sh` 不支持新模块 | 脚本只发 4 个 service SDK | 改用 `publish-maven-release.sh` |

---

## 10. 相关文档

- [`./java-integration`](./java-integration)：Java SDK 用法（消费者向）
- [`./rust-sdk`](./rust-sdk)：Rust SDK 用法（对照参考）
- [`./rust-sdk-publishing`](./rust-sdk-publishing)：Rust SDK 发布流程（与本文档结构对齐）
- [`sdk/java/README.md`](../../sdk/java/README.md)：SDK 模块列表与生成说明
- [`scripts/publish-maven-release.sh`](../../scripts/publish-maven-release.sh)：主发布脚本（release，`curl` 模式）
- [`scripts/publish-maven-snapshot.sh`](../../scripts/publish-maven-snapshot.sh)：主发布脚本（snapshot，`curl` 模式）
- [`scripts/build-java-sdk-docker.js`](../../scripts/build-java-sdk-docker.js)：Docker 容器发布脚本（`mvn deploy` 模式，无需本机 JDK）
- [`scripts/publish-sdks.sh`](../../scripts/publish-sdks.sh)：旧 4 模块脚本（保留兼容）
- [`sdk/java/Dockerfile.builder`](../../sdk/java/Dockerfile.builder)：Docker 模式下的 builder 镜像
- [`release/dist-template/README.md`](../../release/dist-template/README.md)：「Java SDK 引入」章节说明业务服务如何通过 `<dependency>` 引用 SDK