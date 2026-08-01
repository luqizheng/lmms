#!/usr/bin/env node
/**
 * release.js — 打包离线分发包
 * 在有网络的环境执行，输出：
 *   release/coder-micro-services-${VERSION}-airgap.tar.gz
 *   release/coder-micro-services-${VERSION}/
 *
 * 等价于原 release/dist-template/scripts/release.sh
 *
 * 用法：
 *   node release/dist-template/scripts/release.js [VERSION] [--no-images|--images] [--help]
 *   EXPORT_IMAGES=0 node release/dist-template/scripts/release.js [VERSION]
 */

const { execSync } = require('child_process');
const { existsSync, mkdirSync, readFileSync, writeFileSync, cpSync, rmSync } = require('fs');
const path = require('path');

// ── 参数解析 ──────────────────────────────────────────────────────────────────
let VERSION = '';
let EXPORT_IMAGES = process.env.EXPORT_IMAGES ? parseInt(process.env.EXPORT_IMAGES, 10) : 1;
let SHOW_HELP = false;

for (const arg of process.argv.slice(2)) {
  switch (arg) {
    case '--no-images': EXPORT_IMAGES = 0; break;
    case '--images':    EXPORT_IMAGES = 1; break;
    case '--help':
    case '-h':          SHOW_HELP = true; break;
    default:
      if (arg.startsWith('-')) {
        console.error(`[release] 未知参数: ${arg}`);
        SHOW_HELP = true;
      } else if (!VERSION) {
        VERSION = arg;
      } else {
        console.error(`[release] 重复的 VERSION 参数: ${arg}`);
        SHOW_HELP = true;
      }
  }
}

if (SHOW_HELP) {
  console.log(`用法: node release/dist-template/scripts/release.js [VERSION] [--no-images|--images]
  VERSION            第一个位置参数，默认 v1.0.0
  --no-images        跳过 docker pull + docker save（仅打配置/骨架包）
  --images           显式导出镜像（默认行为）
  --help, -h         打印帮助

环境变量：
  REGISTRY           镜像仓库，默认 zhcoder-docker-registry.com:8000
  EXPORT_IMAGES      1=导出镜像（默认），0=跳过`);
  process.exit(0);
}

VERSION = VERSION || 'v1.0.0';
const REGISTRY = process.env.REGISTRY || 'zhcoder-docker-registry.com:8000';

// ── 路径 ─────────────────────────────────────────────────────────────────────
const ROOT = path.resolve(__dirname, '..');          // release/dist-template/
const REPO_ROOT = path.resolve(ROOT, '..', '..');    // repo root
const DIST_DIR = path.join(ROOT, 'release', `coder-micro-services-${VERSION}`);
const DIST_ARCHIVE = path.join(ROOT, 'release', `coder-micro-services-${VERSION}-airgap.tar.gz`);
const IMAGES_DIR = path.join(DIST_DIR, 'images');

// ── 前置检查 ──────────────────────────────────────────────────────────────────
function checkDep(name) {
  try {
    execSync(`command -v ${name}`, { stdio: 'pipe' });
  } catch {
    console.error(`[release] ✗ missing prerequisite: ${name}`);
    return false;
  }
  return true;
}

let missing = 0;
if (!checkDep('node')) missing++;
if (!checkDep('tar'))  missing++;
if (EXPORT_IMAGES === 1 && !checkDep('docker')) missing++;
if (missing > 0) {
  console.error('[release] fix the above and re-run');
  process.exit(1);
}

// ── 镜像清单 ──────────────────────────────────────────────────────────────────
const IMAGES = [
  `${REGISTRY}/micro-server/api-gateway:${VERSION}`,
  `${REGISTRY}/micro-server/config-service:${VERSION}`,
  `${REGISTRY}/micro-server/file-service:${VERSION}`,
  `${REGISTRY}/micro-server/token-service:${VERSION}`,
  `${REGISTRY}/micro-server/iam-service:${VERSION}`,
  `${REGISTRY}/micro-server/notification-service:${VERSION}`,
  `${REGISTRY}/micro-server/audit-service:${VERSION}`,
  `${REGISTRY}/micro-server/prue-admin-backend:${VERSION}`,
  `${REGISTRY}/micro-server/frontend:${VERSION}`,
  'hashicorp/consul:2.0.1',
  'gliderlabs/registrator:master',
  'redis:7-alpine',
  'mysql:8.0',
  'rabbitmq:3.13-management-alpine',
  'prom/prometheus:v2.51.0',
  'grafana/grafana:10.4.0',
  'hashicorp/consul-template:0.32.0',
  'otel/opentelemetry-collector-contrib:0.96.0',
  'jaegertracing/all-in-one:1.55',
];

// ── 工具函数 ──────────────────────────────────────────────────────────────────
function sh(cmd, opts = {}) {
  try {
    return execSync(cmd, { stdio: opts.silent ? 'pipe' : 'inherit', ...opts });
  } catch (e) {
    if (!opts.ignoreError) throw e;
  }
}

function cp(a, b) {
  if (!existsSync(a)) return;
  const stat = require('fs').statSync(a);
  if (stat.isDirectory()) {
    cpSync(a, b, { recursive: true });
  } else {
    const destDir = path.dirname(b);
    if (!existsSync(destDir)) mkdirSync(destDir, { recursive: true });
    require('fs').copyFileSync(a, b);
  }
}

// ── 清理并准备 DIST ──────────────────────────────────────────────────────────
console.log(`[release] VERSION=${VERSION} REGISTRY=${REGISTRY} EXPORT_IMAGES=${EXPORT_IMAGES}`);

if (existsSync(DIST_DIR)) {
  console.log(`[release] wiping stale ${DIST_DIR}`);
  rmSync(DIST_DIR, { recursive: true, force: true });
}
mkdirSync(DIST_DIR, { recursive: true });
if (EXPORT_IMAGES === 1) {
  mkdirSync(IMAGES_DIR, { recursive: true });
}

// ── 拉取并导出镜像 ────────────────────────────────────────────────────────────
if (EXPORT_IMAGES === 1) {
  console.log('[release] -> 拉取所有镜像...');
  for (const img of IMAGES) {
    console.log(`[release]   pull ${img}`);
    sh(`docker pull "${img}"`, { ignoreError: true });
  }

  console.log('[release] -> 导出所有镜像为 tar...');
  for (const img of IMAGES) {
    const fname = img.replace(/[/:]/g, '__');
    console.log(`[release]   save ${img} -> ${fname}.tar`);
    try {
      sh(`docker save "${img}" -o "${path.join(IMAGES_DIR, `${fname}.tar`)}"`);
      console.log(`[release]   ✓ ${fname}.tar`);
    } catch {
      console.log(`[release]   ✗ save failed: ${img}`);
    }
  }
} else {
  console.log('[release] ⚠ --no-images：跳过 docker pull/save');
}

// ── 同步 dist-template 骨架 ───────────────────────────────────────────────────
console.log('[release] -> 同步分发包运行时骨架...');
const distEntries = require('fs').readdirSync(ROOT);
for (const entry of distEntries) {
  if (['scripts', 'release', 'images', 'secrets'].includes(entry)) continue;
  const src = path.join(ROOT, entry);
  const dst = path.join(DIST_DIR, entry);
  cp(src, dst);
}
mkdirSync(path.join(DIST_DIR, 'secrets'), { recursive: true });
const gitkeep = path.join(DIST_DIR, 'secrets', '.gitkeep');
if (!existsSync(gitkeep)) writeFileSync(gitkeep, '');
console.log('[release]   ✓ dist-template skeleton copied');

// ── 同步开发骨架 ──────────────────────────────────────────────────────────────
console.log('[release] -> 同步开发骨架（contracts/scripts/skills/frontend/services/infrastructure）...');
console.log('[release]   ⚠ libs/ skipped - 17 crates published to private kellnr registry');
console.log('[release]   ⚠ sdk/ skipped — sdk/java -> Maven, sdk/rust -> kellnr');

// contracts/
if (existsSync(path.join(REPO_ROOT, 'contracts'))) {
  cp(path.join(REPO_ROOT, 'contracts'), path.join(DIST_DIR, 'contracts'));
  console.log('[release]   ✓ contracts/');
}

// scripts/
if (existsSync(path.join(REPO_ROOT, 'scripts'))) {
  cp(path.join(REPO_ROOT, 'scripts'), path.join(DIST_DIR, 'scripts'));
  console.log('[release]   ✓ scripts/');
  // 从分发包中移除 demo/seed/mock 数据文件。
  // 这些文件仅供开发环境使用，下游仓库各自维护自己的 data-demo/ 目录。
  // 移除它们避免 cms-sync three-way-merge 时覆盖下游自定义的 demo 数据。
  for (const f of ['demo-data.js', 'mock-iam-server.js']) {
    const p = path.join(DIST_DIR, 'scripts', f);
    if (existsSync(p)) rmSync(p, { force: true });
  }
  console.log('[release]   ⚠ removed demo/seed/mock scripts (downstream maintains data-demo/)');
}

// tests/e2e
if (existsSync(path.join(REPO_ROOT, 'tests', 'e2e'))) {
  mkdirSync(path.join(DIST_DIR, 'tests'), { recursive: true });
  cp(path.join(REPO_ROOT, 'tests', 'e2e'), path.join(DIST_DIR, 'tests', 'e2e'));
  // 清理 node_modules / test-results / playwright-report
  for (const dir of ['node_modules', 'test-results', 'playwright-report']) {
    const p = path.join(DIST_DIR, 'tests', 'e2e', dir);
    if (existsSync(p)) rmSync(p, { recursive: true, force: true });
  }
  // 清理报告文件
  const reportGlob = path.join(DIST_DIR, 'tests', 'e2e', 'shared', 'report');
  if (existsSync(reportGlob)) {
    try { rmSync(path.join(reportGlob, 'report-*.xlsx'), { force: true }); } catch {}
  }
  const lastErr = path.join(DIST_DIR, 'tests', 'e2e', 'ui', 'dev', 'last-error.png');
  try { rmSync(lastErr, { force: true }); } catch {}
  const errDir = path.join(DIST_DIR, 'tests', 'e2e', 'shared', 'report', 'errors');
  if (existsSync(errDir)) {
    for (const f of require('fs').readdirSync(errDir)) {
      rmSync(path.join(errDir, f), { force: true });
    }
  }
  console.log('[release]   ✓ tests/e2e/ (node_modules / reports / playwright artifacts excluded)');
}

// .agents/skills
if (existsSync(path.join(REPO_ROOT, '.agents', 'skills'))) {
  mkdirSync(path.join(DIST_DIR, '.agents'), { recursive: true });
  cp(path.join(REPO_ROOT, '.agents', 'skills'), path.join(DIST_DIR, '.agents', 'skills'));
  console.log('[release]   ✓ .agents/skills/');
}

// .agents/rules (优先 .agents/rules，其次 .trae/rules)
if (existsSync(path.join(REPO_ROOT, '.agents', 'rules'))) {
  mkdirSync(path.join(DIST_DIR, '.agents'), { recursive: true });
  cp(path.join(REPO_ROOT, '.agents', 'rules'), path.join(DIST_DIR, '.agents', 'rules'));
  console.log('[release]   ✓ .agents/rules/');
} else if (existsSync(path.join(REPO_ROOT, '.trae', 'rules'))) {
  mkdirSync(path.join(DIST_DIR, '.agents', 'rules'), { recursive: true });
  cp(path.join(REPO_ROOT, '.trae', 'rules'), path.join(DIST_DIR, '.agents', 'rules'));
  console.log('[release]   ✓ .agents/rules/ (migrated from .trae/rules/)');
}

// frontend/
if (existsSync(path.join(REPO_ROOT, 'frontend'))) {
  cp(path.join(REPO_ROOT, 'frontend'), path.join(DIST_DIR, 'frontend'));
  // 清理不需要的目录和文件
  for (const dir of ['node_modules', 'dist', 'dist-ssr', 'coverage']) {
    const p = path.join(DIST_DIR, 'frontend', dir);
    if (existsSync(p)) rmSync(p, { recursive: true, force: true });
  }
  for (const file of ['.eslintcache', 'report.html']) {
    try {
      const walk = (dir) => {
        for (const entry of require('fs').readdirSync(dir, { withFileTypes: true })) {
          const full = path.join(dir, entry.name);
          if (entry.isDirectory()) walk(full);
          else if (entry.name === file || entry.name.endsWith('.local')) rmSync(full, { force: true });
        }
      };
      walk(path.join(DIST_DIR, 'frontend'));
    } catch {}
  }
  console.log('[release]   ✓ frontend/ (node_modules / dist / dist-ssr / coverage / lint-cache excluded)');
} else {
  console.log('[release]   ⚠ frontend/ not found — 前端容器镜像缺失时将无法本地构建');
}

// services/ (empty skeleton)
mkdirSync(path.join(DIST_DIR, 'services'), { recursive: true });
const agentsTmpl = path.join(REPO_ROOT, 'services', 'agents-template.md');
if (existsSync(agentsTmpl)) {
  cp(agentsTmpl, path.join(DIST_DIR, 'services', 'agents-template.md'));
}
const svcGitkeep = path.join(DIST_DIR, 'services', '.gitkeep');
if (!existsSync(svcGitkeep)) writeFileSync(svcGitkeep, '');
console.log('[release]   ✓ services/ (empty - for user-created business services)');

// ── infrastructure/ ──────────────────────────────────────────────────────────
if (!existsSync(path.join(REPO_ROOT, 'infrastructure'))) {
  console.error(`[release]   ✗ infrastructure/ missing at ${path.join(REPO_ROOT, 'infrastructure')}`);
  process.exit(1);
}

mkdirSync(path.join(DIST_DIR, 'infrastructure'), { recursive: true });

// 复制 compose 文件
for (const f of [
  'docker-compose.yml',
  'docker-compose-infra.yml',
  'docker-compose-infra.dev.yml',
  'docker-compose-services.yml',
  'docker-compose-observability.yml',
]) {
  const src = path.join(REPO_ROOT, 'infrastructure', f);
  if (existsSync(src)) cp(src, path.join(DIST_DIR, 'infrastructure', f));
}
console.log('[release]   ✓ infrastructure/docker-compose*.yml');

// transform-compose.js
const svcCompose = path.join(DIST_DIR, 'infrastructure', 'docker-compose-services.yml');
if (!existsSync(svcCompose)) {
  console.error('[release]   ✗ docker-compose-services.yml missing after copy');
  process.exit(1);
}
sh(`node "${path.join(ROOT, 'scripts', 'transform-compose.js')}" "${svcCompose}" "${REGISTRY}" "${VERSION}"`);
console.log('[release]   ✓ docker-compose-services.yml: build → image 转换完成');

// 卷重映射 + 配置复制
const SERVICE_SLUGS = {
  'iam-service': 'iam',
  'notification-service': 'notification',
  'token-service': 'token',
};

const servicesCsv = [];
const slugMapCsv = [];
for (const svcDir of require('fs').readdirSync(path.join(REPO_ROOT, 'services'), { withFileTypes: true })) {
  if (!svcDir.isDirectory()) continue;
  const svc = svcDir.name;
  const configDir = path.join(REPO_ROOT, 'services', svc, 'config');
  if (!existsSync(configDir)) continue;
  const slug = SERVICE_SLUGS[svc] || svc.replace(/-service$/, '');
  servicesCsv.push(svc);
  slugMapCsv.push(`${svc}=${slug}`);
}

if (servicesCsv.length > 0) {
  const remapCmd = `node "${path.join(ROOT, 'scripts', 'release-transforms.js')}" remap-volume \
    --file "${svcCompose}" \
    --services "${servicesCsv.join(',')}" \
    --slug-map "${slugMapCsv.join(',')}"`;
  const remapReport = sh(remapCmd, { silent: true }).toString().trim();

  const changes = JSON.parse(remapReport);
  for (const { svc, slug, matches } of changes.changes) {
    const volPath = `../services/${svc}/config → ./config/${slug}`;
    if (matches > 0) {
      console.log(`[release]   ✓ ${svc} volume: ${volPath} (${matches} occurrences)`);
    } else {
      console.log(`[release]   ⚠ ${svc}: no dev mount found in compose — skipping rewrite`);
    }
  }

  // 复制各服务的 config 目录
  for (const svcDir of require('fs').readdirSync(path.join(REPO_ROOT, 'services'), { withFileTypes: true })) {
    if (!svcDir.isDirectory()) continue;
    const svc = svcDir.name;
    const configDir = path.join(REPO_ROOT, 'services', svc, 'config');
    if (!existsSync(configDir)) continue;
    const slug = SERVICE_SLUGS[svc] || svc.replace(/-service$/, '');
    const destConfig = path.join(DIST_DIR, 'infrastructure', 'config', slug);
    mkdirSync(destConfig, { recursive: true });
    cp(configDir, destConfig);
    // cp -a 的语义：复制目录内容到目标
    for (const entry of require('fs').readdirSync(configDir)) {
      cp(path.join(configDir, entry), path.join(destConfig, entry));
    }
    console.log(`[release]   ✓ infrastructure/config/${slug}/ (from services/${svc}/config/)`);
  }
}

// 非 seed 配置目录
if (existsSync(path.join(REPO_ROOT, 'infrastructure', 'config'))) {
  for (const entry of require('fs').readdirSync(path.join(REPO_ROOT, 'infrastructure', 'config'), { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const name = entry.name;
    const destConfig = path.join(DIST_DIR, 'infrastructure', 'config', name);
    // 跳过已经被 seed config 覆盖的目录
    if (existsSync(destConfig) && existsSync(path.join(destConfig, 'config.default.yaml'))) continue;
    cp(path.join(REPO_ROOT, 'infrastructure', 'config', name), destConfig);
  }
  console.log('[release]   ✓ infrastructure/config/ (non-seed dirs)');
}

// 其他子目录
for (const d of ['mysql-init', 'nginx', 'vendor']) {
  const src = path.join(REPO_ROOT, 'infrastructure', d);
  if (existsSync(src)) {
    cp(src, path.join(DIST_DIR, 'infrastructure', d));
    console.log(`[release]   ✓ infrastructure/${d}/`);
  }
}

console.log('[release]   ✓ infrastructure/ (Docker Compose + configs — ready for dev environment)');

// ── Rust / node workspace stubs ──────────────────────────────────────────────
if (existsSync(path.join(REPO_ROOT, 'Cargo.toml'))) {
  sh(`node "${path.join(ROOT, 'scripts', 'release-transforms.js')}" filter-cargo-members \
    --in "${path.join(REPO_ROOT, 'Cargo.toml')}" \
    --out "${path.join(DIST_DIR, 'Cargo.toml')}"`);
  console.log('[release]   ✓ Cargo.toml (workspace: services/ + sdk/rust/ + libs/ excluded)');
}

mkdirSync(path.join(DIST_DIR, '.cargo'), { recursive: true });
writeFileSync(path.join(DIST_DIR, '.cargo', 'config.toml'), `[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[source.kellnr]
registry = "sparse+http://192.168.2.12:16444/api/v1/crates/"

[net]
git-fetch-with-cli = true
`);
console.log('[release]   ✓ .cargo/config.toml (kellnr registry for libs/* + sdk/rust/*)');

if (existsSync(path.join(REPO_ROOT, 'pnpm-workspace.yaml'))) {
  cp(path.join(REPO_ROOT, 'pnpm-workspace.yaml'), path.join(DIST_DIR, 'pnpm-workspace.yaml'));
}
if (existsSync(path.join(REPO_ROOT, 'package.json'))) {
  cp(path.join(REPO_ROOT, 'package.json'), path.join(DIST_DIR, 'package.json'));
}
console.log('[release]   ✓ pnpm-workspace.yaml + package.json');

if (existsSync(path.join(REPO_ROOT, 'AGENTS.md'))) {
  cp(path.join(REPO_ROOT, 'AGENTS.md'), path.join(DIST_DIR, 'AGENTS.md'));
  sh(`node "${path.join(ROOT, 'scripts', 'release-transforms.js')}" rewrite-agents \
    --file "${path.join(DIST_DIR, 'AGENTS.md')}"`);
  console.log("[release]   ✓ AGENTS.md (.trae/ references replaced with .agents/)");
}

// ── MANIFEST.json ────────────────────────────────────────────────────────────
let GIT_SHA = 'unknown';
let GIT_BRANCH = 'unknown';
try {
  GIT_SHA = execSync('git rev-parse HEAD', { cwd: REPO_ROOT, stdio: 'pipe' }).toString().trim();
} catch {}
try {
  GIT_BRANCH = execSync('git rev-parse --abbrev-ref HEAD', { cwd: REPO_ROOT, stdio: 'pipe' }).toString().trim();
} catch {}

const BUILT_AT = new Date().toISOString().replace(/\.\d{3}Z$/, 'Z');

// 写入临时 images 列表文件
const imagesListFile = path.join(require('os').tmpdir(), `release-images-${Date.now()}.txt`);
writeFileSync(imagesListFile, IMAGES.join('\n'));

sh(`node "${path.join(ROOT, 'scripts', 'release-transforms.js')}" manifest \
  --out "${path.join(DIST_DIR, 'MANIFEST.json')}" \
  --version "${VERSION}" \
  --registry "${REGISTRY}" \
  --built-at "${BUILT_AT}" \
  --git-sha "${GIT_SHA}" \
  --git-branch "${GIT_BRANCH}" \
  --export-images "${EXPORT_IMAGES}" \
  --images-file "${imagesListFile}"`);
rmSync(imagesListFile, { force: true });
console.log(`[release]   ✓ MANIFEST.json (${VERSION} @ ${GIT_SHA.substring(0, 7)})`);

console.log('[release] ✓ 开发骨架同步完成\n');

// ── tar ──────────────────────────────────────────────────────────────────────
console.log('[release] -> 打包 tar.gz...');
const releaseDir = path.join(ROOT, 'release');
const coderDir = `coder-micro-services-${VERSION}`;
sh(`tar -czf "coder-micro-services-${VERSION}-airgap.tar.gz" "${coderDir}/"`, { cwd: releaseDir });

console.log('[release] ✓ 完成！\n');
console.log('输出文件：');
sh(`ls -lh "${DIST_DIR}/"`, { cwd: releaseDir });
sh(`ls -lh "${DIST_ARCHIVE}"`, { cwd: releaseDir });
console.log('');

if (EXPORT_IMAGES === 1) {
  console.log('离线安装步骤：');
  console.log(`  1. 解压：tar -xzf coder-micro-services-${VERSION}-airgap.tar.gz`);
  console.log('  2. 加载镜像：bash bin/load-images.sh');
  console.log('  3. 初始化：make secrets');
  console.log('  4. 启动：make up');
} else {
  console.log('⚠ 本次未导出镜像，离线安装需先自行 docker pull 或单独执行：');
  console.log(`    EXPORT_IMAGES=1 node release/dist-template/scripts/release.js ${VERSION}`);
}
console.log('');
console.log('文档：分发包 docs/ 目录内有 Markdown 源可直接查看；');
console.log('      完整开发文档请参见开发仓库 docs-site/ 目录。');
