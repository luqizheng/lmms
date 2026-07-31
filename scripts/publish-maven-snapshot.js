#!/usr/bin/env node
// scripts/publish-maven-snapshot.js
//
// 把 sdk/java/ 下 Maven 多模块发到 Reposilite 的 *snapshots* 仓库。
//
// 用法（REQUIRED: --version 必须传，且必须带 -SNAPSHOT 后缀）:
//   node scripts/publish-maven-snapshot.js --version 0.4.1-SNAPSHOT
//   node scripts/publish-maven-snapshot.js --version 0.4.1-SNAPSHOT --dry-run
//   node scripts/publish-maven-snapshot.js --help              # 看完整选项
//
// 行为：与 publish-maven-release.js 相同，但目标仓库是 snapshots，--version 必须
//      带 -SNAPSHOT 后缀（Reposilite snapshot policy 强制）。
//
// 凭证 / 仓库 URL 来源：优先 CLI 参数 > env > setup-wizard 配置 > ~/.m2/settings.xml。
// 详细文档：docs-site/guides/java-sdk-publishing.md

'use strict';

const { runPublish } = require('./lib/maven-publish.js');

runPublish('snapshot', process.argv);