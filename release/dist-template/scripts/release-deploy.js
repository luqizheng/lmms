#!/usr/bin/env node
/**
 * release-deploy.js — 打包"纯部署"分发包
 * 不含 dev 骨架（frontend 源码、services、contracts、scripts/skills、Cargo.toml 等）。
 * 默认包含所有 docker 镜像 tar + 已 image: 化的 compose + deploy-only bin 脚本。
 *
 * 与 release.js 的区别：
 *   release.js       → coder-micro-services-v1.0.0-airgap.tar.gz    （开发+部署一体）
 *   release-deploy.js → coder-micro-services-v1.0.0-deploy.tar.gz    （纯部署，无 dev 工具）
 *
 * 用法：
 *   node release/dist-template/scripts/release-deploy.js [VERSION] [--no-images|--images]
 *   EXPORT_IMAGES=0 node release/dist-template/scripts/release-deploy.js [VERSION]
 *
 * 环境变量：
 *   REGISTRY           镜像仓库，默认 zhcoder-docker-registry.com:8000
 *   EXPORT_IMAGES      1=导出镜像（默认），0=跳过 docker build/save
 *
 * 行为：
 *   阶段一（EXPORT_IMAGES=1）：调 build-docker.js -v <ver> --no-push 构建+加载所有镜像到本地
 *   阶段二：拷贝 infrastructure/，跑 transform-compose 处理 services.yml + 主 docker-compose.yml
 *           （主文件的 frontend build → image，吸收 deploy.sh 内联 bash 逻辑）
 *   阶段三（EXPORT_IMAGES=1）：扫所有 compose 文件的 image: 引用，docker save 成 tar
 *   阶段四：拷贝 deploy-only 子集（bin 白名单 + Makefile + docs/deployment + .env.example + secrets/）
 *           + 嵌入发布脚本本体（release-deploy.js + transform-compose.js + release-transforms.js）
 *   写 MANIFEST.json（target=deploy），tar -czf 打 coder-micro-services-${VERSION}-deploy.tar.gz
 */

const { execSync } = require('node:child_process');
const {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  rmSync,
  readdirSync,
  statSync,
  cpSync,
  copyFileSync,
} = require('node:fs');
const path = require('node:path');

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
        console.error(`[release-deploy] 未知参数: ${arg}`);
        SHOW_HELP = true;
      } else if (!VERSION) {
        VERSION = arg;
      } else {
        console.error(`[release-deploy] 重复的 VERSION 参数: ${arg}`);
        SHOW_HELP = true;
      }
  }
}

if (SHOW_HELP) {
  console.log(`用法: node release-deploy.js [VERSION] [--no-images|--images]
  VERSION            第一个位置参数，默认 v1.0.0
  --no-images        跳过 docker build + save（仅打 compose + config + bin 骨架）
  --images           显式导出镜像（默认行为）
  --help, -h         打印帮助

环境变量：
  REGISTRY           镜像仓库，默认 zhcoder-docker-registry.com:8000
  EXPORT_IMAGES      1=导出镜像（默认），0=跳过`);
  process.exit(0);
}

VERSION = VERSION || 'v1.0.0';
const REGISTRY = process.env.REGISTRY || 'zhcoder-docker-registry.com:8000';

// ── 路径 ──────────────────────────────────────────────────────────────────────
const ROOT = path.resolve(__dirname, '..');            // release/dist-template/
const REPO_ROOT = path.resolve(ROOT, '..', '..');      // repo root
const DIST_DIR = path.join(ROOT, 'release', `coder-micro-services-${VERSION}-deploy`);
const DIST_ARCHIVE = path.join(ROOT, 'release', `coder-micro-services-${VERSION}-deploy.tar.gz`);
const IMAGES_DIR = path.join(DIST_DIR, 'images');

// ── 前置检查 ──────────────────────────────────────────────────────────────────
function checkDep(name) {
  // POSIX: which; Windows: where。command -v 是 shell builtin，
  // 在 Windows cmd.exe 上必失败，在 /bin/sh + nvm 装的 node 下
  // 也可能因 PATH 没继承而失败，改用外部命令 + PATH 手动遍历 fallback。
  const cmd = process.platform === 'win32' ? 'where' : 'which';
  try {
    execSync(`${cmd} ${name}`, { stdio: 'pipe' });
    return true;
  } catch {
    const pathEnv = process.env.PATH || '';
    const sep = process.platform === 'win32' ? ';' : ':';
    const exts = process.platform === 'win32' ? ['.exe', '.cmd', '.bat', ''] : [''];
    for (const dir of pathEnv.split(sep)) {
      for (const ext of exts) {
        if (existsSync(path.join(dir, name + ext))) return true;
      }
    }
    console.error(`[release-deploy] ✗ missing prerequisite: ${name}`);
    return false;
  }
}

let missing = 0;
if (!checkDep('node')) missing++;
if (!checkDep('tar')) missing++;
if (EXPORT_IMAGES === 1 && !checkDep('docker')) missing++;
if (missing > 0) {
  console.error('[release-deploy] fix the above and re-run');
  process.exit(1);
}

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
  const stat = statSync(a);
  if (stat.isDirectory()) {
    cpSync(a, b, { recursive: true });
  } else {
    const destDir = path.dirname(b);
    if (!existsSync(destDir)) mkdirSync(destDir, { recursive: true });
    copyFileSync(a, b);
  }
}

function copyIfExists(src, dst, label) {
  if (!existsSync(src)) return false;
  cp(src, dst);
  if (label) console.log(`[release-deploy]   ✓ ${label}`);
  return true;
}

// ── 清理并准备 DIST ──────────────────────────────────────────────────────────
console.log(`[release-deploy] VERSION=${VERSION} REGISTRY=${REGISTRY} EXPORT_IMAGES=${EXPORT_IMAGES}`);

if (existsSync(DIST_DIR)) {
  console.log(`[release-deploy] wiping stale ${DIST_DIR}`);
  rmSync(DIST_DIR, { recursive: true, force: true });
}
mkdirSync(DIST_DIR, { recursive: true });
mkdirSync(path.join(DIST_DIR, 'infrastructure'), { recursive: true });
if (EXPORT_IMAGES === 1) mkdirSync(IMAGES_DIR, { recursive: true });

// ══════════════════════════════════════════════════════════════════════════════
// 阶段一：build + load 镜像到本地（不推仓库，由其他 CI 脚本负责 push）
// ══════════════════════════════════════════════════════════════════════════════
if (EXPORT_IMAGES === 1) {
  console.log('[release-deploy] -> 阶段一：构建所有核心服务镜像（含 frontend）到本地...');
  const buildDocker = path.join(REPO_ROOT, 'scripts', 'build-docker.js');
  try {
    sh(`node "${buildDocker}" -v "${VERSION}" --no-push`);
  } catch (e) {
    console.error('[release-deploy] ✗ build-docker.js 失败');
    console.error('[release-deploy]   hint: 确保 docker daemon 跑着；如在 CI 内确保 Runner 提供 docker socket');
    process.exit(1);
  }
  console.log('[release-deploy]   ✓ 所有镜像已 build + load 到本地 docker（不推仓库）');
} else {
  console.log('[release-deploy] ⚠ --no-images：跳过阶段一（docker build）；消费方需自备镜像');
}

// ══════════════════════════════════════════════════════════════════════════════
// 阶段二：拷贝 infrastructure/ + 跑 transform-compose（处理 services.yml + 主 compose）
// ══════════════════════════════════════════════════════════════════════════════
console.log('[release-deploy] -> 阶段二：拷贝 infrastructure/ + 转换 compose...');

const REPO_INFRA = path.join(REPO_ROOT, 'infrastructure');
if (!existsSync(REPO_INFRA)) {
  console.error(`[release-deploy] ✗ infrastructure/ 不存在: ${REPO_INFRA}`);
  process.exit(1);
}

const COMPOSE_FILES = [
  'docker-compose.yml',
  'docker-compose-infra.yml',
  'docker-compose-infra.dev.yml',
  'docker-compose-services.yml',
  'docker-compose-observability.yml',
];

for (const f of COMPOSE_FILES) {
  copyIfExists(path.join(REPO_INFRA, f), path.join(DIST_DIR, 'infrastructure', f),
    `infrastructure/${f}`);
}

const svcCompose = path.join(DIST_DIR, 'infrastructure', 'docker-compose-services.yml');
const mainCompose = path.join(DIST_DIR, 'infrastructure', 'docker-compose.yml');

if (!existsSync(svcCompose)) {
  console.error('[release-deploy]   ✗ docker-compose-services.yml 缺失（拷贝失败？）');
  process.exit(1);
}
sh(`node "${path.join(ROOT, 'scripts', 'transform-compose.js')}" "${svcCompose}" "${REGISTRY}" "${VERSION}"`);
console.log('[release-deploy]   ✓ docker-compose-services.yml: build → image 转换完成（8 核心 + 2 outbox）');

if (existsSync(mainCompose)) {
  sh(`node "${path.join(ROOT, 'scripts', 'transform-compose.js')}" "${mainCompose}" "${REGISTRY}" "${VERSION}"`);
  console.log('[release-deploy]   ✓ docker-compose.yml: frontend build → image 转换完成');
} else {
  console.log('[release-deploy]   ⚠ docker-compose.yml 不存在，跳过 frontend 转换');
}

// ── config/ 重映射 ──
const SERVICE_SLUGS = {
  'iam-service': 'iam',
  'notification-service': 'notification',
  'token-service': 'token',
};

const servicesCsv = [];
const slugMapCsv = [];
for (const svcDir of readdirSync(path.join(REPO_ROOT, 'services'), { withFileTypes: true })) {
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
  const report = sh(remapCmd, { silent: true }).toString().trim();
  const changes = JSON.parse(report);
  for (const { svc, matches } of changes.changes) {
    if (matches > 0) {
      console.log(`[release-deploy]   ✓ ${svc} volume → ./config/${slugMapCsv.find(s => s.startsWith(svc + '=')).split('=')[1]} (${matches} occurrences)`);
    } else {
      console.log(`[release-deploy]   ⚠ ${svc}: compose 无 dev mount，跳过重写`);
    }
  }

  for (const svcDir of readdirSync(path.join(REPO_ROOT, 'services'), { withFileTypes: true })) {
    if (!svcDir.isDirectory()) continue;
    const svc = svcDir.name;
    const configDir = path.join(REPO_ROOT, 'services', svc, 'config');
    if (!existsSync(configDir)) continue;
    const slug = SERVICE_SLUGS[svc] || svc.replace(/-service$/, '');
    const destConfig = path.join(DIST_DIR, 'infrastructure', 'config', slug);
    mkdirSync(destConfig, { recursive: true });
    cp(configDir, destConfig);
    for (const entry of readdirSync(configDir)) {
      cp(path.join(configDir, entry), path.join(destConfig, entry));
    }
    console.log(`[release-deploy]   ✓ infrastructure/config/${slug}/ (from services/${svc}/config/)`);
  }
}

if (existsSync(path.join(REPO_INFRA, 'config'))) {
  for (const entry of readdirSync(path.join(REPO_INFRA, 'config'), { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const name = entry.name;
    const destConfig = path.join(DIST_DIR, 'infrastructure', 'config', name);
    if (existsSync(destConfig) && existsSync(path.join(destConfig, 'config.default.yaml'))) continue;
    cp(path.join(REPO_INFRA, 'config', name), destConfig);
  }
  console.log('[release-deploy]   ✓ infrastructure/config/ (非 seed 目录)');
}

for (const d of ['mysql-init', 'nginx', 'scripts', 'vendor']) {
  copyIfExists(path.join(REPO_INFRA, d), path.join(DIST_DIR, 'infrastructure', d),
    `infrastructure/${d}/`);
}

// ══════════════════════════════════════════════════════════════════════════════
// 阶段三：扫所有 compose 中的 image: → docker save 成 tar
// ══════════════════════════════════════════════════════════════════════════════
let ALL_IMAGES = [];
let SAVED_COUNT = 0;
let SKIPPED_COUNT = 0;
let MISSING_COUNT = 0;

if (EXPORT_IMAGES === 1) {
  console.log('[release-deploy] -> 阶段三：扫描 compose 中所有 image: 引用 → docker save...');

  const imageSet = new Set();
  for (const f of COMPOSE_FILES) {
    const composePath = path.join(DIST_DIR, 'infrastructure', f);
    if (!existsSync(composePath)) continue;
    const text = readFileSync(composePath, 'utf8');
    for (const m of text.matchAll(/^[[:space:]]+image:[[:space:]]+(.+)$/gm)) {
      const img = m[1].trim().replace(/^["']|["']$/g, '');
      if (img.includes('${')) continue;   // skip ${VAR} refs
      if (img.startsWith('#')) continue;
      if (!img) continue;
      imageSet.add(img);
    }
  }
  ALL_IMAGES = [...imageSet].sort();
  console.log(`[release-deploy]   → ${ALL_IMAGES.length} 个 image: 引用去重后`);

  for (const img of ALL_IMAGES) {
    const fname = img.replace(/[/:]/g, '__') + '.tar';
    const outPath = path.join(IMAGES_DIR, fname);

    if (existsSync(outPath)) {
      SKIPPED_COUNT++;
      continue;
    }

    try {
      execSync(`docker inspect "${img}" >/dev/null 2>&1`, { stdio: 'pipe' });
    } catch {
      console.log(`[release-deploy]   ⚠ ${img} 本地 docker 不存在，跳过（消费方需自备）`);
      MISSING_COUNT++;
      continue;
    }

    try {
      sh(`docker save "${img}" -o "${outPath}"`, { silent: true });
      SAVED_COUNT++;
      console.log(`[release-deploy]     ✓ ${img} → ${fname}`);
    } catch {
      console.error(`[release-deploy]     ✗ save 失败: ${img}`);
      MISSING_COUNT++;
    }
  }
  console.log(`[release-deploy]   ✓ 镜像: save=${SAVED_COUNT} cached=${SKIPPED_COUNT} missing=${MISSING_COUNT}`);

  // images/manifest.json
  const manifestEntries = ALL_IMAGES.map(img => {
    const fname = img.replace(/[/:]/g, '__') + '.tar';
    const tarPath = path.join(IMAGES_DIR, fname);
    return {
      image: img,
      tar: existsSync(tarPath) ? fname : null,
      size_bytes: existsSync(tarPath) ? statSync(tarPath).size : null,
    };
  });
  writeFileSync(path.join(IMAGES_DIR, 'manifest.json'), JSON.stringify({
    target: 'deploy',
    version: VERSION,
    registry: REGISTRY,
    images: manifestEntries,
  }, null, 2));
  console.log(`[release-deploy]   ✓ images/manifest.json`);
} else {
  console.log('[release-deploy] ⚠ --no-images：跳过阶段三');
}

// ══════════════════════════════════════════════════════════════════════════════
// 阶段四：拷贝 deploy-only 子集 (infrastructure 之外的"运行时附件")
// ══════════════════════════════════════════════════════════════════════════════
console.log('[release-deploy] -> 阶段四：拷贝 deploy-only 子集...');

// bin/ 白名单：deploy 包只需要 ops 操作脚本，不需要 dev 测试 / seed / build
const BIN_DEPLOY_WHITELIST = [
  'generate-secrets.js',
  'load-images.js',
  'logs.js',
  'start.js',           // 已加 --deploy flag，跳过 build-frontend
  'status.js',
  'stop.js',
  'upgrade.js',
  'verify.js',
];
const BIN_SOURCE = path.join(ROOT, 'bin');
const BIN_DEST = path.join(DIST_DIR, 'bin');
mkdirSync(BIN_DEST, { recursive: true });
let copiedBin = 0;
for (const f of BIN_DEPLOY_WHITELIST) {
  const src = path.join(BIN_SOURCE, f);
  if (existsSync(src)) {
    copyFileSync(src, path.join(BIN_DEST, f));
    copiedBin++;
  } else {
    console.error(`[release-deploy]   ⚠ bin/${f} 不存在`);
  }
}
console.log(`[release-deploy]   ✓ bin/ (${copiedBin}/${BIN_DEPLOY_WHITELIST.length} 个 deploy-needed scripts)`);

// Makefile (deploy 专用)
const deployMakefile = `.PHONY: help secrets up down status logs verify load-images upgrade

REGISTRY ?= ${REGISTRY}
IMAGE_TAG ?= ${VERSION}

help:
\t@echo "coder-micro-services \${IMAGE_TAG} — 纯部署包 (来自 release-deploy.js)"
\t@echo ""
\t@echo "  make secrets         生成 JWT 密钥 + 随机密码，初始化 .env"
\t@echo "  make up              启动所有服务（用预构建镜像，无 build-frontend 步骤）"
\t@echo "  make down            停止容器（保留数据卷）"
\t@echo "  make status          列出容器状态"
\t@echo "  make logs [svc=]     查看日志"
\t@echo "  make verify          端到端冒烟测试"
\t@echo "  make load-images     从离线 tar 加载镜像（air-gap）"
\t@echo "  make upgrade TAG=vX.Y.Z 升级到指定版本"
\t@echo ""
\t@echo "环境变量：REGISTRY=\${REGISTRY} IMAGE_TAG=\${IMAGE_TAG}"

secrets:
\t@node bin/generate-secrets.js

up:
\t@node bin/start.js --deploy

down:
\t@node bin/stop.js

status:
\t@node bin/status.js

logs:
\t@node bin/logs.js \$(svc)

verify:
\t@node bin/verify.js

load-images:
\t@node bin/load-images.js

upgrade:
\t@node bin/upgrade.js \$(TAG)
`;
writeFileSync(path.join(DIST_DIR, 'Makefile'), deployMakefile);
console.log('[release-deploy]   ✓ Makefile (deploy 专用)');

// docs/ 仅部署相关
mkdirSync(path.join(DIST_DIR, 'docs'), { recursive: true });
copyIfExists(path.join(ROOT, 'docs', 'deployment'),
             path.join(DIST_DIR, 'docs', 'deployment'),
             'docs/deployment/');
copyIfExists(path.join(ROOT, 'docs', 'getting-started.md'),
             path.join(DIST_DIR, 'docs', 'getting-started.md'),
             'docs/getting-started.md');

// .env.example
copyIfExists(path.join(ROOT, '.env.example'), path.join(DIST_DIR, '.env.example'),
             '.env.example');

// secrets/ 空目录（运维运行时由 make secrets 填充）
mkdirSync(path.join(DIST_DIR, 'secrets'), { recursive: true });
writeFileSync(path.join(DIST_DIR, 'secrets', '.gitkeep'), '');

// README.md (deploy 专用，README 在 docs-site 里有更详细版本)
writeFileSync(path.join(DIST_DIR, 'README.md'), `# coder-micro-services ${VERSION} — 纯部署包

> 由 \`release/dist-template/scripts/release-deploy.js\` 在 dev 仓库内直接产出。
> 与 airgap 包（\`coder-micro-services-${VERSION}-airgap.tar.gz\`）的区别：
>   - **不含** dev 工具（frontend 源码、services 骨架、contracts/skills、scripts/build-docker.js 等）
>   - **包含** 预构建的 Docker 镜像 tar + compose + 精简 ops 脚本
>   - 体积约为 airgap 包的 1/3（无 node_modules / 无 skills / 无 frontend 源码）

## 快速部署

\`\`\`bash
# 离线：先 load 镜像
node bin/load-images.js

# 在线：直接 up（registry 已可访问）
node bin/generate-secrets.js    # 生成 JWT 密钥 + 随机密码
make up                          # 启动所有服务
make verify                      # 端到端冒烟测试
\`\`\`

## 访问地址（默认端口）

| 服务 | 地址 | 说明 |
| --- | --- | --- |
| 前端 | http://localhost:3000 | Vue 3（nginx 镜像）|
| API 网关 | http://localhost:8080 | 唯一 HTTP 入口 |
| Consul | http://localhost:8500 | 服务发现 |
| Grafana | http://localhost:3001 | admin / admin |
| Prometheus | http://localhost:9090 | 指标查询 |
| Jaeger | http://localhost:16686 | Trace UI |

## 升级

\`\`\`bash
make upgrade TAG=v1.0.X
\`\`\`

## 与 airgap 包的关系

本包 vs airgap 包：

| 维度 | airgap（\`release.js\` 产物） | deploy（\`release-deploy.js\` 产物）|
| --- | --- | --- |
| 前端源码 | ✅ 含（\`frontend/\`） | ❌ 已预构建进镜像 |
| services 骨架 | ✅ 空目录 + skill 模板 | ❌ 无 |
| 镜像 tar | ✅（需消费方 load） | ✅（需消费方 load）|
| 二次开发能力 | ✅ 用 skill 自助创建业务服务 | ❌ 仅部署 |
| 体积 | ~ 大（含 frontend + skills）| ~ 小（仅 ops）|

如果需要二次开发能力，请同时拉 **airgap 包**。本包只面向"拿到了 tag、给个 compose、起服务"的运维场景。
`);
console.log('[release-deploy]   ✓ README.md (deploy 专用)');

// ── 嵌入发布脚本本体（让 deploy 包可以"再打 patch"，无需回到 dev 仓库克隆）──
const SCRIPTS_SRC = path.join(ROOT, 'scripts');
const SCRIPTS_DST = path.join(DIST_DIR, 'scripts');
mkdirSync(SCRIPTS_DST, { recursive: true });

// 自嵌入 + 关键依赖（transform-compose + release-transforms）
const SELF_EMBED = [
  'release-deploy.js',
  'transform-compose.js',
  'release-transforms.js',
];
let embedded = 0;
for (const f of SELF_EMBED) {
  const src = path.join(SCRIPTS_SRC, f);
  if (existsSync(src)) {
    copyFileSync(src, path.join(SCRIPTS_DST, f));
    embedded++;
  } else {
    console.error(`[release-deploy]   ⚠ scripts/${f} 不存在，跳过自嵌入`);
  }
}
console.log(`[release-deploy]   ✓ scripts/ (${embedded} 个自嵌入发布脚本：${SELF_EMBED.join(', ')})`);

// CHANGELOG.md（部署包也带，方便运维查版本变更）
copyIfExists(path.join(ROOT, 'CHANGELOG.md'), path.join(DIST_DIR, 'CHANGELOG.md'),
             'CHANGELOG.md');

// ══════════════════════════════════════════════════════════════════════════════
// MANIFEST.json
// ══════════════════════════════════════════════════════════════════════════════
let GIT_SHA = 'unknown';
let GIT_BRANCH = 'unknown';
try {
  GIT_SHA = execSync('git rev-parse HEAD', { cwd: REPO_ROOT, stdio: 'pipe' }).toString().trim();
} catch {}
try {
  GIT_BRANCH = execSync('git rev-parse --abbrev-ref HEAD', { cwd: REPO_ROOT, stdio: 'pipe' }).toString().trim();
} catch {}

const BUILT_AT = new Date().toISOString().replace(/\.\d{3}Z$/, 'Z');

writeFileSync(path.join(DIST_DIR, 'MANIFEST.json'), JSON.stringify({
  target: 'deploy',                                  // 区分 release.js 产物的 'airgap'
  staging_swap_target: VERSION,                      // deploy:staging job 读这个字段做 atomic symlink swap；与 version 相同但语义明确
  version: VERSION,
  registry: REGISTRY,
  built_at: BUILT_AT,
  git_sha: GIT_SHA,
  git_branch: GIT_BRANCH,
  export_images: EXPORT_IMAGES,
  images_count: ALL_IMAGES.length,
  images: ALL_IMAGES,
}, null, 2));
console.log(`[release-deploy]   ✓ MANIFEST.json (target=deploy, staging_swap_target=${VERSION}, @ ${GIT_SHA.substring(0, 7)})`);

// ══════════════════════════════════════════════════════════════════════════════
// 打包 tar.gz
// ══════════════════════════════════════════════════════════════════════════════
console.log('[release-deploy] -> 打包 tar.gz...');
const releaseDir = path.join(ROOT, 'release');
const coderDir = `coder-micro-services-${VERSION}-deploy`;
// 不带结尾 /：Windows bsdtar 把 "dir/" 解析成 dir + 空字符串两参数，
// 报 "tar: : Couldn't visit directory"；GNU tar (Linux/macOS) 对不带 / 的目录参数等价。
sh(`tar -czf "coder-micro-services-${VERSION}-deploy.tar.gz" "${coderDir}"`,
   { cwd: releaseDir });

console.log('[release-deploy] ✓ 完成！\n');
console.log('输出：');
// 跨平台：Windows 用 dir，POSIX 用 ls -lh（Windows 无 ls 命令）
const listCmd = process.platform === 'win32' ? 'dir' : 'ls -lh';
sh(`${listCmd} "${DIST_DIR}"`, { cwd: releaseDir });
sh(`${listCmd} "${DIST_ARCHIVE}"`, { cwd: releaseDir });

if (EXPORT_IMAGES === 1) {
  console.log('\n部署步骤：');
  console.log(`  1. tar -xzf coder-micro-services-${VERSION}-deploy.tar.gz`);
  console.log(`  2. cd coder-micro-services-${VERSION}-deploy`);
  console.log(`  3. node bin/load-images.js`);
  console.log(`  4. node bin/generate-secrets.js`);
  console.log(`  5. make up        （内部: node bin/start.js --deploy）`);
} else {
  console.log(`\n⚠ 本次未导出镜像（--no-images），需消费方自备镜像。`);
  console.log(`   重跑带镜像版本：node ${path.relative(REPO_ROOT, __filename)} ${VERSION}`);
}
