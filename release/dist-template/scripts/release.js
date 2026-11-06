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
let ENV_FILE = '';

for (const arg of process.argv.slice(2)) {
  switch (arg) {
    case '--no-images': EXPORT_IMAGES = 0; break;
    case '--images':    EXPORT_IMAGES = 1; break;
    case '--help':
    case '-h':          SHOW_HELP = true; break;
    default:
      if (arg.startsWith('--env-file=')) {
        ENV_FILE = arg.slice('--env-file='.length);
      } else if (arg.startsWith('-')) {
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
  console.log(`用法: node release/dist-template/scripts/release.js [VERSION] [--no-images|--images] [--env-file=PATH]
  VERSION            第一个位置参数，默认 v1.0.0
  --no-images        跳过 docker pull + docker save（仅打配置/骨架包）
  --images           显式导出镜像（默认行为）
  --env-file=PATH    指定镜像源变量来源（默认用 release/dist-template/.env.example）
  --help, -h         打印帮助

环境变量：
  REGISTRY           镜像仓库（写入 MANIFEST），默认 zhcoder-docker-registry.com:8000
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
    console.error(`[release] ✗ missing prerequisite: ${name}`);
    return false;
  }
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
// 镜像名不再硬编码在此：compose 里所有 image: 都是 ${VAR} 形式（换源只改 .env），
// 必须扫 compose 并解析变量才能得到最终镜像名 —— 否则会拉错镜像或漏拉。
// 共享模块与 release-deploy.js / load-images.js 共用，保证三条打包路径产物一致。
const { scanComposeImages } = require('./image-resolver.js');

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
// 注意：本段必须在 compose 复制 + transform 之后执行（见下方「导出镜像」段）。
// 原因：改造后镜像名不再硬编码，而是扫产物里的 compose 解析 ${VAR} 得出；
// 而 services.yml 在 transform 之前只有 build: 块、没有 image: 行，提前扫会漏掉
// 全部平台镜像 —— 这正是旧实现「静默产出缺镜像的包」的根因之一。
if (EXPORT_IMAGES === 0) {
  console.log('[release] ⚠ --no-images：跳过 docker pull/save');
}

// ── 同步 dist-template 骨架 ───────────────────────────────────────────────────
console.log('[release] -> 同步分发包运行时骨架...');
const distEntries = require('fs').readdirSync(ROOT);
for (const entry of distEntries) {
  // 'scripts' 故意保留排除：上面 readdir 循环会递归自我嵌入 release.js 自身，
  // 改用下方"显式 cp(ROOT/scripts, DIST_DIR/scripts)"段单独同步 release 脚本。
  if (['scripts', 'release', 'images', 'secrets'].includes(entry)) continue;
  const src = path.join(ROOT, entry);
  const dst = path.join(DIST_DIR, entry);
  cp(src, dst);
}
mkdirSync(path.join(DIST_DIR, 'secrets'), { recursive: true });
const gitkeep = path.join(DIST_DIR, 'secrets', '.gitkeep');
if (!existsSync(gitkeep)) writeFileSync(gitkeep, '');
console.log('[release]   ✓ dist-template skeleton copied');

// ── 版本号注入：dist 内 Makefile / .env.example 的 IMAGE_TAG 对齐包版本 ─────────
// 源头模板恒为 1.0.0；打包时把真实版本写入产物。配合 transform-compose.js 生成的
// ${IMAGE_TAG:-<version>} 插值，make up 用包内默认值，bin/upgrade.js 升级时以
// 进程环境变量 / .env 覆盖，两处版本永远有唯一出处。
const IMAGE_TAG_VALUE = VERSION.replace(/^v/, '');
function rewriteImageTag(relFile, applyFns) {
  const abs = path.join(DIST_DIR, relFile);
  if (!existsSync(abs)) return;
  const before = readFileSync(abs, 'utf-8');
  let after = before;
  for (const fn of applyFns) after = fn(after);
  if (after !== before) {
    writeFileSync(abs, after);
    console.log(`[release]   ✓ ${relFile}: IMAGE_TAG → ${IMAGE_TAG_VALUE}`);
  }
}
rewriteImageTag('.env.example', [
  (t) => t.replace(/^IMAGE_TAG=.*$/m, `IMAGE_TAG=${IMAGE_TAG_VALUE}`),
]);
rewriteImageTag('Makefile', [
  (t) => t.replace(/^IMAGE_TAG[ \t]*\??=.*$/m, `IMAGE_TAG ?= ${IMAGE_TAG_VALUE}`),
  (t) => t.replace(/(coder-micro-services )v\d+(?:\.\d+)*/, `$1v${IMAGE_TAG_VALUE}`),
]);

// ── 同步发布脚本子集（绕过上述 readdir 循环的 scripts 排除，让 airgap 产物能复打）────
// 'scripts' 在上文骨架同步被显式排除，原因：readdirSync(ROOT) 含 'scripts' 自身，
// 若递归拷贝会让 release.js 把自己的副本嵌入自己的副本里。
// 这里以"显式单次 cp"绕过：源 = release/dist-template/scripts（一个目录），
// 目标 = DIST_DIR/scripts（尚未存在），只复制 release.js / transform-compose.js /
// release-transforms.js / release-deploy.js 等发布脚本，不涉及 readdir 循环的递归。
// 下游新项目可复用它给后续版本重打离线包；已有项目的版本更新走包升级
// （镜像 compose pull + pnpm/cargo/maven 升版本），不再做源码级同步。
const RELEASE_SCRIPTS_SRC = path.join(ROOT, 'scripts');
const RELEASE_SCRIPTS_DST = path.join(DIST_DIR, 'scripts');
if (existsSync(RELEASE_SCRIPTS_SRC)) {
  cp(RELEASE_SCRIPTS_SRC, RELEASE_SCRIPTS_DST);
  console.log(`[release]   ✓ dist-template/scripts/ → DIST_DIR/scripts/ (含 ${require('fs').readdirSync(RELEASE_SCRIPTS_DST).filter(f => !f.endsWith('.test.js')).join(', ')})`);
}

// ── 同步开发骨架 ──────────────────────────────────────────────────────────────
console.log('[release] -> 同步开发骨架（contracts/packages/scripts/skills/services/infrastructure）...');
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
  // 移除它们避免下游各自维护的 demo 数据被发布包覆盖。
  for (const f of ['demo-data.js', 'mock-iam-server.js']) {
    const p = path.join(DIST_DIR, 'scripts', f);
    if (existsSync(p)) rmSync(p, { force: true });
  }
  console.log('[release]   ⚠ removed demo/seed/mock scripts (downstream maintains data-demo/)');
}

// tests/e2e
if (existsSync(path.join(REPO_ROOT, 'tests', 'e2e'))) {
  mkdirSync(path.join(DIST_DIR, 'tests'), { recursive: true });
  // 使用 filter 在复制时跳过 node_modules / test-results / playwright-report 等重目录，
  // 避免「先全量复制再删除」在 Windows 上因海量小文件复制极慢的问题。
  const E2E_EXCLUDE_DIRS = new Set(['node_modules', 'test-results', 'playwright-report']);
  cpSync(path.join(REPO_ROOT, 'tests', 'e2e'), path.join(DIST_DIR, 'tests', 'e2e'), {
    recursive: true,
    filter: (src) => {
      const name = path.basename(src);
      if (E2E_EXCLUDE_DIRS.has(name)) return false;
      // 跳过报告文件
      if (name === 'last-error.png' || /^report-.*\.xlsx$/.test(name)) return false;
      return true;
    },
  });
  // 清理 errors 目录下的报告文件（filter 只跳过文件名匹配，errors 目录本身保留）
  const errDir = path.join(DIST_DIR, 'tests', 'e2e', 'shared', 'report', 'errors');
  if (existsSync(errDir)) {
    for (const f of require('fs').readdirSync(errDir)) {
      rmSync(path.join(errDir, f), { force: true });
    }
  }
  console.log('[release]   ✓ tests/e2e/ (node_modules / reports / playwright artifacts excluded)');
}

// .agents/skills — 下游白名单（单一真相源：本仓 .agents/skills）
//
// v1.x 排序：分发包只携带下游创建/维护业务微服务要用的 scaffold skill，
// repo-local（SDK 发布 / CI 诊断 / 平台工程…）一概不封包，避免误用本仓发布技能。
// 增加条目即可把新 skill 纳入分发（仍需同步统一 dist README 的描述）。
const DOWNSTREAM_SKILL_WHITELIST = [
  // Rust 业务服务脚手架
  'coder-new-service',
  'coder-add-route',
  'coder-migrate-db',
  // Java 业务服务脚手架
  'coder-new-java-service',
  'coder-add-java-route',
  'coder-migrate-java-db',
  // PRISM 前端（admin-ui）
  'prism-design',
  'scaffold',
  'update-shell',
  'gen-page',
];

(function syncDownstreamSkills() {
  const skillsSrcDir = path.join(REPO_ROOT, '.agents', 'skills');
  const skillsDstDir = path.join(DIST_DIR, '.agents', 'skills');
  if (!existsSync(skillsSrcDir)) return;
  mkdirSync(skillsDstDir, { recursive: true });

  let copied = 0;
  for (const name of DOWNSTREAM_SKILL_WHITELIST) {
    const src = path.join(skillsSrcDir, name);
    if (!existsSync(src)) {
      console.log(`[release]   ⚠ whitelisted skill missing: ${name}`);
      continue;
    }
    cp(src, path.join(skillsDstDir, name));
    copied++;
  }
  console.log(`[release]   ✓ .agents/skills/ (白名单 ${copied}/${DOWNSTREAM_SKILL_WHITELIST.length} 个 scaffold)`);

  // 说明：其余 repo-local skill 属框架仓，不外发。
  const readmePath = path.join(DIST_DIR, '.agents', 'README.downstream.md');
  writeFileSync(
    readmePath,
`# 本包 .agents/skills 说明（下游开发用）

本分发包的 \`.agents/skills/\` 仅包含下游「创建 / 维护 Java / Rust 业务微服务」与
「开发 PRISM admin 前端」要用的 scaffold：

| skill | 语言 | 用途 |
| --- | --- | --- |
| coder-new-service | Rust | 新建 Rust 业务微服务骨架（8-item 规范） |
| coder-add-route | Rust | Rust 服务加路由 / OpenAPI |
| coder-migrate-db | Rust | Rust 双（MySQL/SQLite）语义迁移 |
| coder-new-java-service | Java | 新建 Java(Spring Boot 3 + JDK 21) 业务微服务骨架 + compose/网关注册 |
| coder-add-java-route | Java | Java 服务加 Controller/DTO |
| coder-migrate-java-db | Java | Java Flyway 迁移 |
| prism-design | 前端 | PRISM 设计系统与前端代码约束（写 UI 前必读） |
| scaffold | 前端 | 用 PRISM 壳初始化新的业务系统前端（拷贝壳 + 装基础包 + 品牌化） |
| update-shell | 前端 | 把上游壳的更新同步到下游（diff + 人工裁决定制区） |
| gen-page | 前端 | 按后端契约生成列表页 / 弹框编辑页 |

其余 repo-local skill（SDK 发布、CI 诊断、性能剖析、平台工程等）属于框架源码仓库自身开发用，**不进本包**。

如你的下游需要的是这些框架侧能力，请到框架开发仓库使用；若确有必要把某个 skill 随包分发给下游，由框架维护者在开发仓库的
\`DOWNSTREAM_SKILL_WHITELIST\`（\`release/dist-template/scripts/release.js\`）追加条目后重新发包。
`
  );
  console.log('[release]   ✓ .agents/README.downstream.md (repo-local skills 不外发说明)');
})();

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

// packages/ — 前端 TS 访问包（@digua/contracts / @digua/http），前端集成入口
if (existsSync(path.join(REPO_ROOT, 'packages'))) {
  const PKG_EXCLUDE_DIRS = new Set(['node_modules', 'dist']);
  cpSync(path.join(REPO_ROOT, 'packages'), path.join(DIST_DIR, 'packages'), {
    recursive: true,
    filter: (src) => !PKG_EXCLUDE_DIRS.has(path.basename(src)),
  });
  console.log('[release]   ✓ packages/ (node_modules / dist excluded)');
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

// 复制 compose 文件 —— 2026-11-06 起源码布局各组进自包含子目录
// （infra/ services/ observability/），dist 包仍保持扁平 infrastructure/ 根
// （bin/*.js、staging 链路、容器命名都骑在扁平布局上）。
for (const [f, srcRel] of Object.entries({
  'docker-compose.yml': 'docker-compose.yml',
  'docker-compose-infra.yml': 'infra/docker-compose-infra.yml',
  'docker-compose-infra.dev.yml': 'infra/docker-compose-infra.dev.yml',
  'docker-compose-services.yml': 'services/docker-compose-services.yml',
  'docker-compose-observability.yml': 'observability/docker-compose-observability.yml',
  // 日志栈片（loki+promtail）：随包发布供 ops 叠加（load-images.js 会扫它拉镜像）
  'docker-compose-observability.logs.yml': 'observability/docker-compose-observability.logs.yml',
})) {
  const src = path.join(REPO_ROOT, 'infrastructure', srcRel);
  if (existsSync(src)) cp(src, path.join(DIST_DIR, 'infrastructure', f));
}
console.log('[release]   ✓ infrastructure/docker-compose*.yml');

// transform-compose.js
const svcCompose = path.join(DIST_DIR, 'infrastructure', 'docker-compose-services.yml');
if (!existsSync(svcCompose)) {
  console.error('[release]   ✗ docker-compose-services.yml missing after copy');
  process.exit(1);
}
sh(`node "${path.join(ROOT, 'scripts', 'transform-compose.js')}" "${svcCompose}" "${VERSION}"`);
console.log('[release]   ✓ docker-compose-services.yml: build → image 转换完成');

// 主 docker-compose.yml 同样要把 build: 块转成 image:（release-deploy.js 也做这件事）
// — 产物 dist 包不带服务源码，build: 会导致 `docker compose up` 现场构建失败。
//   现与 release-deploy.js 行为对齐：两个 compose 文件都跑 transform-compose。
const mainCompose = path.join(DIST_DIR, 'infrastructure', 'docker-compose.yml');
if (existsSync(mainCompose)) {
  sh(`node "${path.join(ROOT, 'scripts', 'transform-compose.js')}" "${mainCompose}" "${VERSION}"`);
  console.log('[release]   ✓ docker-compose.yml: build → image 转换完成');
}

// ── 导出镜像（必须在 compose 转换之后）────────────────────────────────────
// 镜像名来自扫产物 compose + 解析 ${VAR}（换源只改 .env，无需重新打包）。
// 变量解析优先级与 compose 一致：进程 env > --env-file > compose 内 :- 默认值。
// 镜像源变量来源：--env-file > 进程 env > 产物 .env.example（默认基准）。
// ⚠ 必须读「产物」的 .env.example 而非模板：上面的 rewriteImageTag 已把产物的
// IMAGE_TAG 改成本次打包版本（不含 v），模板里仍是 1.0.0 —— 读模板会导致
// 解析出的镜像 tag 与 compose 展开值不一致（MANIFEST 与实际拉的镜像对不上）。
const DIST_ENV_EXAMPLE = path.join(DIST_DIR, '.env.example');
const RESOLVE_ENV_FILE = ENV_FILE
  ? path.resolve(ENV_FILE)
  : (existsSync(DIST_ENV_EXAMPLE) ? DIST_ENV_EXAMPLE : path.join(ROOT, '.env.example'));
if (ENV_FILE) {
  console.log(`[release] → 镜像源变量来源: ${RESOLVE_ENV_FILE}`);
}
const { images: RESOLVED_IMAGES, unresolved: UNRESOLVED_VARS } = scanComposeImages(
  path.join(DIST_DIR, 'infrastructure'),
  { envFile: RESOLVE_ENV_FILE, env: process.env }
);

if (UNRESOLVED_VARS.length > 0) {
  console.error(`[release] ✗ compose 存在无法解析的镜像变量: ${UNRESOLVED_VARS.join(', ')}`);
  console.error('[release]   请在 .env（或 --env-file）补齐这些变量，或给 compose 里的引用加 :- 默认值');
  process.exit(1);
}

console.log(`[release] → 解析出 ${RESOLVED_IMAGES.length} 个镜像引用`);

if (EXPORT_IMAGES === 1) {
  console.log('[release] -> 拉取所有镜像...');
  for (const img of RESOLVED_IMAGES) {
    console.log(`[release]   pull ${img}`);
    // pull 失败必须中断：旧实现用 ignoreError 静默吞掉，导致 air-gap 包
    // 静默缺失镜像却「看起来成功」（离线端才暴露缺镜像）。
    try {
      sh(`docker pull "${img}"`);
    } catch {
      console.error(`[release] ✗ pull 失败: ${img}`);
      console.error('[release]   打包中止：镜像不完整会导致离线安装缺镜像');
      process.exit(1);
    }
  }

  console.log('[release] -> 导出所有镜像为 tar...');
  let saved = 0;
  for (const img of RESOLVED_IMAGES) {
    const fname = img.replace(/[/:]/g, '__');
    console.log(`[release]   save ${img} -> ${fname}.tar`);
    try {
      sh(`docker save "${img}" -o "${path.join(IMAGES_DIR, `${fname}.tar`)}"`);
      console.log(`[release]   ✓ ${fname}.tar`);
      saved++;
    } catch {
      console.error(`[release] ✗ save 失败: ${img}`);
      console.error('[release]   打包中止：镜像不完整会导致离线安装缺镜像');
      process.exit(1);
    }
  }

  // 完整性闸门：实际产出的 tar 数必须与期望镜像数一致
  if (saved !== RESOLVED_IMAGES.length) {
    console.error(`[release] ✗ 镜像导出数量不符: 期望 ${RESOLVED_IMAGES.length}，实际 ${saved}`);
    process.exit(1);
  }
  console.log(`[release] ✓ 镜像导出完整 (${saved}/${RESOLVED_IMAGES.length})`);
}

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

// 非 seed 配置目录 —— 源已按组分区（infra/ services/ observability/ 各自的 config/），
// 逐组合并进 bundle 的扁平 config/；兼容历史扁平布局（存在则仍拷）。
const GROUP_CONFIG_SRCS = ['infra', 'services', 'observability']
  .map((g) => path.join(REPO_ROOT, 'infrastructure', g, 'config'));
GROUP_CONFIG_SRCS.push(path.join(REPO_ROOT, 'infrastructure', 'config'));
let copiedConfigGroups = 0;
for (const groupConfig of GROUP_CONFIG_SRCS) {
  if (!existsSync(groupConfig)) continue;
  for (const entry of require('fs').readdirSync(groupConfig, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const name = entry.name;
    const destConfig = path.join(DIST_DIR, 'infrastructure', 'config', name);
    // 跳过已经被 seed config 覆盖的目录
    if (existsSync(destConfig) && existsSync(path.join(destConfig, 'config.default.yaml'))) continue;
    cp(path.join(groupConfig, name), destConfig);
  }
  copiedConfigGroups++;
}
if (copiedConfigGroups > 0) {
  console.log('[release]   ✓ infrastructure/config/ (non-seed dirs, group-partitioned)');
}

// 其他子目录（mysql-init 已随 infra 组进子目录）
for (const [d, srcRel] of Object.entries({
  'mysql-init': 'infra/mysql-init',
  'nginx': 'nginx',
  'vendor': 'vendor',
})) {
  const src = path.join(REPO_ROOT, 'infrastructure', srcRel);
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
    --file "${path.join(DIST_DIR, 'AGENTS.md')}" \
    --downstream-skills "${DOWNSTREAM_SKILL_WHITELIST.join(',')}"`);
  console.log("[release]   ✓ AGENTS.md (.trae/ replaced with .agents/; non-whitelisted skill rows dropped)");
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
writeFileSync(imagesListFile, RESOLVED_IMAGES.join('\n'));

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
// 不带结尾 /：Windows bsdtar 把 "dir/" 解析成 dir + 空字符串两参数，
// 报 "tar: : Couldn't visit directory"；GNU tar (Linux/macOS) 对不带 / 的目录参数等价。
sh(`tar -czf "coder-micro-services-${VERSION}-airgap.tar.gz" "${coderDir}"`, { cwd: releaseDir });

console.log('[release] ✓ 完成！\n');
console.log('输出文件：');
// 跨平台：Windows 用 dir，POSIX 用 ls -lh（Windows 无 ls 命令）
const listCmd = process.platform === 'win32' ? 'dir' : 'ls -lh';
sh(`${listCmd} "${DIST_DIR}"`, { cwd: releaseDir });
sh(`${listCmd} "${DIST_ARCHIVE}"`, { cwd: releaseDir });
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
