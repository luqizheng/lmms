# LMMS Java SDK Maven Central 发布设计

日期：2026-11-27  
状态：实施中（PR review）

## 已确认决策

| 项目 | 决策 |
|---|---|
| Namespace / groupId | `io.github.luqizheng` |
| 发布方式 | 上传后人工审核、人工 Publish |
| GPG | 当前未准备 |
| Central 凭据 | 已有 `MVN_USERNAME`、`MVN_PASSWORD` |
| 发布触发 | 仅严格符合 `vMAJOR.MINOR.PATCH` 的 tag |
| PR / branch | 只验证，不上传 |
| 正式发布 | 本 PR 不创建 tag、不上传 deployment、不点击 Publish |

## 坐标迁移

父 POM、七个子模块 parent 坐标及模块间依赖统一迁移为 `io.github.luqizheng`。Java 源码 package `io.digua.*` 保持不变。内部 Reposilite 已有旧坐标不覆盖、不删除。

## 发布要求

- 七个 JAR 模块附带主 JAR、Sources JAR、Javadoc JAR 和 GPG detached signature。
- 父 POM 提供项目名称、描述、URL、Apache-2.0 License、Developer 和 SCM 元数据。
- Central 插件只在显式 `central-publishing` profile 启用，配置 `autoPublish=false`。
- GPG 只在 `central-release` profile 启用。
- PR workflow 仅执行 Java build/test/verify。
- Tag workflow 校验严格 semver 与 POM version 一致，再签名上传。
- 不修改 Rust、npm、Docker 发布工作流。
- GPG secrets 缺失时必须在上传前失败。

## 验收标准

1. 所有 Maven groupId 和内部依赖坐标统一。
2. 默认构建不触发 GPG 或 Central 上传。
3. PR 不上传；只有 v-prefixed semver tag 可上传。
4. 上传后必须人工在 Sonatype Central Publish。
5. 文档包含 Namespace 检查、GPG key 生成、Secrets、Tag 发布及人工 Publish。
