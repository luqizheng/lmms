#!/usr/bin/env node
/**
 * scripts/release-staging.js — 生成 pre-staging 预发布目录
 *
 * 单一职责：把 infrastructure/ + compose（build → image 转换）+ deploy 脚本
 * 打成 self-contained 目录，CI 或人工拿去 rsync 到 staging 起服。
 *
 * 明确**不**做的事（其他组件负责）：
 *   - 构建/推送镜像 → scripts/build-docker.js
 *   - rsync / SSH 部署 → CI 的 deploy:pre-staging 任务，或人工手动
 *   - 离线 airgap 包 → release/dist-template/scripts/release.js
 *
 * 用法：
 *   node scripts/release-staging.js [--commit <sha>] [--output <dir>]
 *                                   [--registry <url>] [--repo <repo>]
 *
 * 默认：
 *   --commit    git rev-parse --short HEAD
 *   --output    release/staging-<commit>/
 *   --registry  zhcoder-docker-registry.com:8000
 *   --repo      micro-server
 *
 * 输出：
 *   <output>/
 *   ├── infrastructure/
 *   │   ├── docker-compose*.yml             (services.yml 已 build → image:<commit>)
 *   │   ├── config/                          (per-service configs)
 *   │   ├── mysql-init/  nginx/  vendor/     (按需)
 *   └── scripts/
 *       └── deploy-staging.sh               (commit-pinned 部署脚本)
 *
 * 完整 CI 编排流程（.gitlab-ci.yml）：
 *   build_images:pre-staging → release:pre-staging → deploy:pre-staging
 *                            （build 镜像    → 打目录        → rsync + SSH）
 *
 * 完整手动流程（CI 太慢 / 临时验证时）：
 *   # 1. 构建并推送所有镜像
 *   node scripts/build-docker.js -v abc1234 --push \
 *       api-gateway token-service iam-service notification-service \
 *       audit-service config-service file-service prue-admin-backend \
 *       frontend
 *
 *   # 2. 生成 pre-staging 目录
 *   node scripts/release-staging.js --commit abc1234 \
 *       --output release/staging-abc1234/
 *
 *   # 3. 部署到 staging
 *   rsync -a release/staging-abc1234/ kudi@192.168.2.128:~/pre-staging/abc1234/
 *   ssh kudi@192.168.2.128 "STAGING_COMMIT=abc1234 bash ~/pre-staging/abc1234/scripts/deploy-staging.sh"
 */

const fs = require('node:fs');
const path = require('node:path');
const { execSync } = require('node:child_process');

const ROOT = path.resolve(__dirname, '..');
const TRANSFORM_SCRIPT = path.join(
  ROOT,
  'release',
  'dist-template',
  'scripts',
  'transform-compose.js',
);

const DEFAULT_REGISTRY = 'zhcoder-docker-registry.com:8000';
const DEFAULT_REPO = 'micro-server';

// ── 参数解析 ────────────────────────────────────────────────────────────────
let commit = '';
let output = '';
let registry = DEFAULT_REGISTRY;
let repo = DEFAULT_REPO;
let showHelp = false;

for (let i = 2; i < process.argv.length; i++) {
  const a = process.argv[i];
  switch (a) {
    case '--commit':   commit = process.argv[++i] || ''; break;
    case '--output':   output = process.argv[++i] || ''; break;
    case '--registry': registry = process.argv[++i] || DEFAULT_REGISTRY; break;
    case '--repo':     repo = process.argv[++i] || DEFAULT_REPO; break;
    case '--help': case '-h': showHelp = true; break;
    default:
      console.error(`[release-staging] 未知参数: ${a}`);
      showHelp = true;
  }
}

if (showHelp) {
  console.log(`用法: node scripts/release-staging.js [options]

生成 pre-staging 预发布目录：复制 infrastructure/ + transform compose (build → image:<commit>)。

明确不做的事：
  - 构建/推送镜像：用 scripts/build-docker.js
  - rsync / SSH 部署：CI 调 deploy:pre-staging，或人工手动

选项：
  --commit <sha>     commit SHA (默认 git rev-parse --short HEAD)
  --output <dir>     输出目录 (默认 release/staging-<commit>/)
  --registry <url>   镜像仓库 (默认 ${DEFAULT_REGISTRY})
  --repo <name>      镜像 repo (默认 ${DEFAULT_REPO})
  --help, -h         打印帮助`);
  process.exit(process.argv.includes('--help') || process.argv.includes('-h') ? 0 : 1);
}

// ── commit SHA 解析 ─────────────────────────────────────────────────────────
if (!commit) {
  try {
    commit = execSync('git rev-parse --short HEAD', { cwd: ROOT, stdio: 'pipe' }).toString().trim();
  } catch (e) {
    console.error('[release-staging] ✗ 无法确定 commit SHA（git rev-parse 失败或 --commit 未提供）');
    process.exit(1);
  }
}
if (!/^[0-9a-f]{4,40}$/i.test(commit)) {
  console.error(`[release-staging] ✗ commit SHA 格式异常: "${commit}"`);
  process.exit(1);
}

if (!output) output = path.join(ROOT, 'release', `staging-${commit}`);
const outputAbs = path.resolve(output);

console.log(`[release-staging] commit=${commit} output=${outputAbs} registry=${registry}/${repo}`);

// ── 清理 + 创建输出目录 ─────────────────────────────────────────────────────
if (fs.existsSync(outputAbs)) {
  console.log(`[release-staging] 清理旧目录 ${outputAbs}`);
  fs.rmSync(outputAbs, { recursive: true, force: true });
}
fs.mkdirSync(outputAbs, { recursive: true });

// ── 复制 infrastructure/ ────────────────────────────────────────────────────
const infraSrc = path.join(ROOT, 'infrastructure');
const infraDst = path.join(outputAbs, 'infrastructure');

if (!fs.existsSync(infraSrc)) {
  console.error(`[release-staging] ✗ infrastructure/ 不存在: ${infraSrc}`);
  process.exit(1);
}

fs.mkdirSync(infraDst, { recursive: true });

const COMPOSE_FILES = [
  'docker-compose.yml',
  'docker-compose-infra.yml',
  'docker-compose-infra.dev.yml',
  'docker-compose-services.yml',
  'docker-compose-observability.yml',
  'docker-compose.prod.yml',
];
let composeCopied = 0;
for (const f of COMPOSE_FILES) {
  const src = path.join(infraSrc, f);
  if (!fs.existsSync(src)) continue;
  fs.copyFileSync(src, path.join(infraDst, f));
  composeCopied++;
}
console.log(`[release-staging] ✓ infrastructure/docker-compose*.yml (${composeCopied} 个)`);

const svcCompose = path.join(infraDst, 'docker-compose-services.yml');
const mainCompose = path.join(infraDst, 'docker-compose.yml');
if (!fs.existsSync(svcCompose)) {
  console.error('[release-staging] ✗ docker-compose-services.yml 不存在（无法 transform）');
  process.exit(1);
}
// 主 docker-compose.yml 也 transform：mysql-replica 与 frontend 含 build: 块需转为 registry image；
// 它已从 docker-compose-infra.yml 迁至主 compose。其余 infra/observability 服务仅声明
// 固定版本的第三方 image:，IMAGE_MAP 不含它们，transform-compose 原样保留。
const composeToTransform = [svcCompose, mainCompose];
for (const f of composeToTransform) {
  if (!fs.existsSync(f)) {
    console.warn(`[release-staging] ⚠ 跳过不存在的 compose: ${path.basename(f)}`);
    continue;
  }
  try {
    execSync(`node "${TRANSFORM_SCRIPT}" "${f}" "${registry}" "${commit}"`, {
      stdio: 'inherit',
    });
    console.log(`[release-staging] ✓ ${path.basename(f)}: build 转为 image: ${registry}/${repo}/<svc>:${commit}`);
  } catch (e) {
    console.error(`[release-staging] ✗ transform-compose 失败: ${path.basename(f)}`);
    process.exit(1);
  }
}

for (const d of ['config', 'mysql-init', 'nginx', 'vendor']) {
  const src = path.join(infraSrc, d);
  if (fs.existsSync(src)) {
    fs.cpSync(src, path.join(infraDst, d), { recursive: true });
    console.log(`[release-staging] ✓ infrastructure/${d}/`);
  }
}

const deployStagingSrc = path.join(infraSrc, 'scripts', 'deploy-staging.sh');
if (!fs.existsSync(deployStagingSrc)) {
  console.error(`[release-staging] ✗ ${deployStagingSrc} 不存在（请先创建）`);
  process.exit(1);
}
const scriptsDst = path.join(outputAbs, 'scripts');
fs.mkdirSync(scriptsDst, { recursive: true });
fs.copyFileSync(deployStagingSrc, path.join(scriptsDst, 'deploy-staging.sh'));
fs.chmodSync(path.join(scriptsDst, 'deploy-staging.sh'), 0o755);
console.log('[release-staging] ✓ scripts/deploy-staging.sh');

console.log(`\n[release-staging] ✓ 完成：${outputAbs}`);