#!/usr/bin/env node
/**
 * load-images.js — 将离线 tar 镜像包加载到本地 Docker（air-gap 安装）
 * 等价于原 release/dist-template/bin/load-images.sh
 */
const { execSync } = require('child_process');
const { existsSync, readdirSync } = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const IMAGES_DIR = path.join(ROOT, 'images');

if (!existsSync(IMAGES_DIR)) {
  console.error(`[load-images] ✗ ${IMAGES_DIR} 不存在，请先放置 .tar 镜像包`);
  process.exit(1);
}

if (!existsSync(path.join(IMAGES_DIR, 'manifest.json'))) {
  console.error('[load-images] ✗ manifest.json 不存在');
  process.exit(1);
}

console.log('[load-images] 开始加载离线镜像...');
for (const entry of readdirSync(IMAGES_DIR)) {
  if (!entry.endsWith('.tar')) continue;
  console.log(`[load-images]   → docker load -i ${entry}`);
  execSync(`docker load -i "${path.join(IMAGES_DIR, entry)}"`, { stdio: 'inherit' });
}
console.log('[load-images] ✓ 加载完成');
