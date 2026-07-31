#!/usr/bin/env node
// scripts/publish-maven-release.js
//
// 把 sdk/java/ 下 Maven 多模块发到 Reposilite 的 *releases* 仓库。
//
// 用法（REQUIRED: --version 必须传）:
//   node scripts/publish-maven-release.js --version 0.4.0
//   node scripts/publish-maven-release.js --version 0.4.0 --profile java17
//   node scripts/publish-maven-release.js --version 0.4.0 --dry-run
//   node scripts/publish-maven-release.js --help              # 看完整选项
//
// 行为：
//   1. 校验 --version <ver>（必须不带 -SNAPSHOT 后缀）
//   2. 自动 `mvn versions:set` 把 sdk/java/pom.xml 的 <version> 节点改到 <ver>（幂等）
//   3. 跑 `mvn install` 把所有模块装到本地 ~/.m2
//   4. curl -T 上传到 Reposilite（绕过 Reposilite 的 Expect:100-continue 兼容性问题）
//   5. ⚠ pom.xml 的 <version> 改动不自动 git commit ——
//      发完后请手动：git diff sdk/java/ && git add sdk/java && git commit -m "feat(sdk): bump to <ver>"
//
// 凭证 / 仓库 URL 来源：优先 CLI 参数 > env > setup-wizard 配置 > ~/.m2/settings.xml。
// 详细文档：docs-site/guides/java-sdk-publishing.md

'use strict';

const { runPublish } = require('./lib/maven-publish.js');

runPublish('release', process.argv);