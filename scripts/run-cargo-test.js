#!/usr/bin/env node
// 本机跑 cargo test 时，让 utoipa-swagger-ui 的 build.rs 使用仓库内置的
// swagger-ui zip，避免每次编译都去 GitHub 下载（见 AGENTS.md）。
const { spawnSync } = require('node:child_process');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..');
const zipPath = path.join(repoRoot, 'infrastructure', 'vendor', 'swagger-ui-v5.17.14.zip');
const fileUrl = 'file:///' + zipPath.replace(/\\/g, '/');

const args = process.argv.slice(2);
if (args.length === 0) {
  args.push('test');
}

const result = spawnSync('cargo', args, {
  stdio: 'inherit',
  env: { ...process.env, SWAGGER_UI_DOWNLOAD_URL: fileUrl },
  shell: process.platform === 'win32',
});

process.exit(result.status ?? 1);
