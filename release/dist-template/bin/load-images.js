#!/usr/bin/env node
/**
 * load-images.js — 将镜像加载到本地 Docker
 *
 * 默认 mode=load：从 ./images/*.tar 加载（air-gap 离线安装）
 * mode=pull    ：从 registry 拉取（在线 staging / dev 同步）
 *
 * mode=pull 时：扫 infrastructure/*.yml 中所有 image: 引用，把 <ver> 替换
 * 为 IMAGE_TAG（默认 v1.0.0；建议传 $CI_COMMIT_SHORT_SHA）后 docker pull。
 * 用于 staging server 上 deploy-staging 流程。
 *
 * 用法：
 *   node bin/load-images.js                          # 默认 load（找 ./images/*.tar）
 *   node bin/load-images.js --mode=pull              # 从 registry 拉所有 image: 引用
 *   node bin/load-images.js --mode=pull --tag=<ver>  # 替换 <ver> 为 <ver> 拉
 *   IMAGE_TAG=v1.1.0 node bin/load-images.js --mode=pull
 */
const { execSync } = require('child_process');
const { existsSync, readdirSync, readFileSync } = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const IMAGES_DIR = path.join(ROOT, 'images');
// 镜像名解析与 release.js / release-deploy.js 共用同一模块，保证三条路径一致。
const { scanComposeImages } = require('../scripts/image-resolver.js');

// ── arg parsing ──
const args = process.argv.slice(2);
let mode = 'load';                  // 'load' | 'pull'
let tagOverride = process.env.IMAGE_TAG || 'v1.0.0';
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === '--mode=pull')  { mode = 'pull'; continue; }
  if (a === '--mode=load')  { mode = 'load'; continue; }
  if (a.startsWith('--tag=')) { tagOverride = a.slice('--tag='.length); continue; }
  if (a === '--tag') { tagOverride = args[++i]; continue; }
  console.error(`[load-images] 未知参数: ${a}`);
  process.exit(2);
}

// ── mode=load: air-gap 安装路径（保持原行为）──
if (mode === 'load') {
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
  process.exit(0);
}

// ── mode=pull: 从 registry 拉所有 compose 中 image: 引用 ──
console.log(`[load-images] mode=pull：扫描 compose 中 image: 引用并 docker pull（tag=${tagOverride}）`);
const INFRA_DIR = path.join(ROOT, 'infrastructure');

// compose 里所有 image: 都是 ${VAR} 形式（换源只改 .env），必须解析变量才能
// 得到最终镜像名。旧的实现用 VAR_INTERPOLATION 白名单（只有 REGISTRY/BASE_REGISTRY）
// 内联插值、且 tag 正则匹配不了 ${IMAGE_TAG:-x}，改造后会漏掉全部平台镜像。
// 现统一走 image-resolver.js，与 release.js / release-deploy.js 同源。
const ENV_FILE_FOR_PULL = process.env.ENV_FILE
  ? path.resolve(process.env.ENV_FILE)
  : path.join(ROOT, '.env');

if (!existsSync(INFRA_DIR)) {
  console.error(`[load-images] ✗ ${INFRA_DIR} 不存在`);
  process.exit(1);
}

const composeFiles = [
  'docker-compose.yml',
  'docker-compose-infra.yml',
  'docker-compose-infra.dev.yml',
  'docker-compose-services.yml',
  'docker-compose-observability.yml',
  // 日志栈片（loki+promtail）：镜像必须进离线包，否则离线端叠日志片拉不到镜像
  'docker-compose-observability.logs.yml',
].filter(f => existsSync(path.join(INFRA_DIR, f)));

if (composeFiles.length === 0) {
  console.error('[load-images] ✗ 没找到任何 compose 文件');
  process.exit(1);
}

const { images: resolvedImages, unresolved: unresolvedVars } = scanComposeImages(INFRA_DIR, {
  files: composeFiles,
  envFile: ENV_FILE_FOR_PULL,
  env: process.env,
});

if (unresolvedVars.length > 0) {
  console.error(`[load-images] ✗ compose 存在无法解析的镜像变量: ${unresolvedVars.join(', ')}`);
  console.error('[load-images]   请在 .env 补齐这些变量，或给 compose 里的引用加 :- 默认值');
  process.exit(1);
}

// tag 归一：镜像 tag 不带 v（CI 推送用 ${CI_COMMIT_TAG#v} 剥 v）。
// 平台镜像的 tag 由 compose 里 ${IMAGE_TAG:-<version>} 决定 —— 用 tagOverride 覆盖，
// 让 CI 按 per-commit SHA 拉平台镜像；第三方镜像的固定版本 tag 不受影响。
const imageSet = new Set(
  resolvedImages.map(img =>
    img.replace(/:v?\d+\.\d+\.\d+(?:[-.][\w.-]+)*$/, ':' + tagOverride)
  )
);

const images = [...imageSet].sort();
console.log(`[load-images] → ${images.length} 个 image: 引用去重后`);

let ok = 0, fail = 0;
for (const img of images) {
  console.log(`[load-images]   → docker pull ${img}`);
  try {
    execSync(`docker pull "${img}"`, { stdio: 'inherit' });
    ok++;
  } catch {
    console.error(`[load-images]   ✗ pull 失败: ${img}`);
    fail++;
  }
}
console.log(`[load-images] ✓ mode=pull 完成 (ok=${ok} fail=${fail})`);
if (fail > 0) process.exit(1);