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

// compose 里基础设施层 / observability 层用 ${BASE_REGISTRY} / ${REGISTRY}
// 占位符（不是字面 registry），load-images 不能像 compose 那样从 .env 读。
// 内置 fallback：CI 已 export 这两个 ENV；本地手跑时用项目惯用值。
//
// 统一保证尾斜杠：
//   - compose 约定 ${BASE_REGISTRY}mysql:8.0（占位符紧贴镜像名，不带额外 /）
//   - 若 CI 变量漏写尾斜杠（如 .../official 而非 .../official/），字符串插值会拼出
//     .../officialmysql 这种畸形引用。normalize 一步消除该类事故。
const REGISTRY_FALLBACK = (process.env.REGISTRY || process.env.DOCKER_REGISTRY || 'zhcoder-docker-registry.com:8000')
  .replace(/\/+$/, '') + '/';
const BASE_REGISTRY_FALLBACK = (process.env.BASE_REGISTRY || (REGISTRY_FALLBACK + 'official'))
  .replace(/\/+$/, '') + '/';
const VAR_INTERPOLATION = {
  REGISTRY: REGISTRY_FALLBACK,
  BASE_REGISTRY: BASE_REGISTRY_FALLBACK,
};
function interpolateVars(raw) {
  return raw.replace(/\$\{([A-Z_][A-Z0-9_]*)\}/g, (m, name) =>
    Object.prototype.hasOwnProperty.call(VAR_INTERPOLATION, name) ? VAR_INTERPOLATION[name] : m
  );
}
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
].filter(f => existsSync(path.join(INFRA_DIR, f)));

if (composeFiles.length === 0) {
  console.error('[load-images] ✗ 没找到任何 compose 文件');
  process.exit(1);
}

// 扫描 image: 引用；用 <ver> 模板替换
const imageSet = new Set();
// 只有业务镜像 compose 里的 image: tag 才替换成 $IMAGE_TAG（per-commit SHA）。
// 这两个文件里的 build: 块在 release 时被 transform-compose.js 转成
// micro-server/<svc>:<version>，正是需要按本次部署 SHA 拉取的平台镜像。
// docker-compose-infra.yml / docker-compose-observability.yml 里全是第三方
// 基础镜像（grafana / consul / prometheus / mysql / redis ...），tag 固定为
// 上游版本号，在镜像仓库里真实存在，绝不能被 SHA 覆盖，否则会去拉
// .../grafana/grafana:<sha> 这种根本不存在的 tag。
const TAG_REPLACE_FILES = new Set([
  'docker-compose.yml',
  'docker-compose-services.yml',
]);
for (const f of composeFiles) {
  const replaceTag = TAG_REPLACE_FILES.has(f);
  const text = readFileSync(path.join(INFRA_DIR, f), 'utf8');
  for (const m of text.matchAll(/^[ \t]+image:[ \t]+(.+)$/gm)) {
    const raw = m[1].trim().replace(/^["']|["']$/g, '');
    if (raw.startsWith('#')) continue;
    // inline 替换已知占位符（REGISTRY / BASE_REGISTRY），未知变量保留原样
    // 后再过滤——防御性，避免漏下的未知占位符污染 image 列表。
    const interpolated = interpolateVars(raw);
    if (interpolated.includes('${')) continue;
    if (replaceTag) {
      // 把 :<ver> 替换成 :tagOverride
      const resolved = interpolated.replace(/:v?\d+\.\d+\.\d+(?:[-.][\w.-]+)*$/, ':' + tagOverride);
      imageSet.add(resolved);
    } else {
      imageSet.add(interpolated);
    }
  }
}

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