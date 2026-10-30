# 内网仓库凭证申请流程

> **状态**：草案 · 待运维补充
> **关联**：ai-context.md §4.2 Onboarding/运维待推进项
> **适用范围**：coder-micro-services 平台所有内部仓库凭证管理

---

## 0. 一句话

> 本项目使用两类内部仓库：Docker Registry、Maven 仓库。
> 本文档说明如何申请、配置、验证各类凭证。

---

## 1. 凭证类型总览

| 仓库类型 | 地址 | 用途 | 凭证类型 |
|----------|------|------|----------|
| Docker Registry | `zhcoder-docker-registry.com:8000` | 推送/拉取服务镜像 | 用户名+密码 或 ServiceAccount |
| Maven 仓库 | `mvn.kudi.repos` | Java SDK 发布/依赖 | 用户名+密码 或 Deploy Token |

---

## 2. Docker Registry 凭证

### 2.1 凭证说明

- **registry 地址**：`zhcoder-docker-registry.com:8000`
- **镜像路径**：`zhcoder-docker-registry.com:8000/micro-server/<service>:<tag>`
- **凭证类型**：
  - 开发环境：个人账号密码
  - 生产环境：ServiceAccount（机器人账号）

### 2.2 申请流程

```
1. 向 DevOps/平台团队发送申请邮件/工单
   内容：
   - 用途说明
   - 需要的权限（read / read-write）
   - 有效期（临时凭证）
   
2. 收到凭证后配置本地 Docker
```

### 2.3 本地配置

```bash
# 1. 登录（交互式）
docker login zhcoder-docker-registry.com:8000 -u <username>

# 2. 登录后凭证存储在 ~/.docker/config.json
cat ~/.docker/config.json
# 应该看到类似：
# {
#   "auths": {
#     "zhcoder-docker-registry.com:8000": {
#       "auth": "<base64-encoded-credentials>"
#     }
#   }
# }

# 3. 验证登录
docker pull zhcoder-docker-registry.com:8000/micro-server/iam-service:latest
```

### 2.4 CI/CD 配置

```bash
# GitLab CI/CD Variables 设置（非明文）
# Settings > CI/CD > Variables
#
# DOCKER_REGISTRY_URL: zhcoder-docker-registry.com:8000
# DOCKER_REGISTRY_USER: <service-account-username>
# DOCKER_REGISTRY_PASSWORD: <service-account-password>

# .gitlab-ci.yml 中使用
- docker login -u $DOCKER_REGISTRY_USER -p $DOCKER_REGISTRY_PASSWORD $DOCKER_REGISTRY_URL
- docker push $DOCKER_REGISTRY_URL/micro-server/$SERVICE:$TAG
```

### 2.5 常见问题

| 问题 | 解决 |
|------|------|
| `401 Unauthorized` | 重新登录，确认用户名密码正确 |
| `connection refused` | 检查网络/代理设置 |
| `certificate signed by unknown authority` | 添加 registry CA 证书 |

---

## 3. Maven 仓库凭证

### 3.1 凭证说明

- **Maven 仓库地址**：`mvn.kudi.repos`（Nexus 或 Artifactory）
- **用途**：
  - 发布 Java SDK（`sdk/java/`）
  - 依赖内部公共库

### 3.2 申请流程

```
1. 向 DevOps/平台团队发送申请
   内容：
   - 项目/SDK 名称
   - 用途（publish / consume）
   - Maven coordinates（groupId、artifactId）

2. 收到凭证后配置 ~/.m2/settings.xml
```

### 3.3 settings.xml 配置

```xml
<!-- ~/.m2/settings.xml -->
<?xml version="1.0" encoding="UTF-8"?>
<settings>
  <servers>
    <!-- 内部 Maven 仓库发布凭证 -->
    <server>
      <id>internal-releases</id>
      <username>${ENV_MAVEN_USER}</username>
      <password>${ENV_MAVEN_PASSWORD}</password>
    </server>
    <server>
      <id>internal-snapshots</id>
      <username>${ENV_MAVEN_USER}</username>
      <password>${ENV_MAVEN_PASSWORD}</password>
    </server>
    <!-- 阿里云 Maven 镜像（如果使用） -->
    <server>
      <id>aliyun-maven</id>
      <username>${ENV_ALIYUN_MAVEN_USER}</username>
      <password>${ENV_ALIYUN_MAVEN_PASSWORD}</password>
    </server>
  </servers>
</settings>
```

### 3.4 pom.xml 配置

```xml
<distributionManagement>
  <repository>
    <id>internal-releases</id>
    <name>Internal Releases</name>
    <url>https://mvn.kudi.repos/releases</url>
  </repository>
  <snapshotRepository>
    <id>internal-snapshots</id>
    <name>Internal Snapshots</name>
    <url>https://mvn.kudi.repos/snapshots</url>
  </snapshotRepository>
</distributionManagement>
```

### 3.5 验证配置

```bash
# 测试依赖下载
mvn dependency:get \
  -DgroupId=io.digua \
  -DartifactId=user-service-user-sdk \
  -Dversion=0.2.0 \
  -DrepoUrl=https://mvn.kudi.repos/releases

# 测试发布（dry-run）
mvn deploy -DskipTests -DaltDeploymentRepository=local::default::file:///tmp/maven-repo
```

---

## 4. Rust Crates 代理

### 5.1 当前配置
项目 `.cargo/config` 已配置 rsproxy.cn 代理：

```toml
# .cargo/config
[source.crates-io]
replace-with = "rsproxy"

[source.rsproxy]
registry = "https://rsproxy.cn/crates.io-index"

[registries.rsproxy]
index = "https://rsproxy.cn/crates.io-index"

[net]
git-fetch-with-cli = true
```

### 5.2 何时需要换源

| 场景 | 操作 |
|------|------|
| 内网无法访问 rsproxy.cn | 改回 crates.io 或使用公司内部源 |
| 同步慢 | 使用 rsproxy.cn 的 git 索引镜像 |
| 需要发布内部 crate | 配置公司内部 crate 仓库 |

---

## 6. GitLab Container Registry

### 6.1 凭证说明

每个 GitLab 项目有自己的 Container Registry：
- 地址：`registry.gitlab.com/<group>/<project>`

### 6.2 Deploy Token 申请

```
GitLab > Settings > Repository > Deploy tokens

创建 Token：
- Name: ci-deploy-token
- Scope: read_registry (拉取) / write_registry (推送)
- Expiration: 90 days
```

### 6.3 CI/CD 使用

```bash
# .gitlab-ci.yml
build:
  image: docker:latest
  services:
    - docker:dind
  before_script:
    - docker login -u $CI_REGISTRY_USER -p $CI_REGISTRY_PASSWORD $CI_REGISTRY
  script:
    - docker build -t $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA .
    - docker push $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA
```

---

## 7. 凭证轮换

### 7.1 轮换原则

| 凭证类型 | 建议轮换频率 | 触发条件 |
|----------|-------------|----------|
| 个人账号 | 180 天 | 人员异动 |
| ServiceAccount | 90 天 | 定期轮换 |
| API Key | 90 天 | 疑似泄漏 |
| Deploy Token | 90 天 | 定期轮换 |

### 7.2 轮换流程

```bash
# 1. 申请新凭证

# 2. 更新存储
# 本地：~/.docker/config.json 或 ~/.m2/settings.xml
# CI：GitLab CI/CD Variables

# 3. 验证新凭证
docker pull <image>
mvn dependency:get <artifact>

# 4. 验证旧凭证失效
docker pull <image>  # 应该失败

# 5. 通知相关人员（如果需要）
```

---

## 8. 关联

- `.cargo/config` — Rust crates 代理配置
- `.gitlab-ci.yml` — CI/CD 凭证使用
- `coder build images` — Docker 镜像构建与推送（`--push` / `--load-and-push`）

---

> **最后更新**：2026-07-13
> **待补充**：
> - 实际仓库地址（联系 DevOps 确认）
> - 具体申请工单入口
> - 审批流程
