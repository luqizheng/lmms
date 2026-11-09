'use strict';

// image-env.js —— 部署单元镜像钉版（.image.env）的生成与解析
//
// 单一实现，供两处复用：
//   - packages/cli/commands/release.js（coder release images 推完自动更新）
//   - scripts/update-image-env.js（独立脚本入口 + registry 探测）
//
// .image.env 是「本系统自建镜像」的唯一真相源：compose 的 image: 只写变量名
// （${IMAGE_FILE_SERVICE}），具体镜像名与 tag 全在这里。设计见
// docs/superpowers/specs/2026-11-09-image-env-driven-deploy-design.md。

const fs = require('node:fs');
const path = require('node:path');

const ENV_FILE = '.image.env';

// ── 服务 → 镜像映射 ──────────────────────────────────────────────────────────
// env   : .image.env 里的变量名（compose 的 image: 引用它）
// image : registry 里的镜像名（≠ compose service 名时在此显式映射）
// dir   : services/ 下的目录名（版本号从 <dir>/Cargo.toml 读）
// db    : 是否带 DB（带 DB 的服务有 pg 方言变体，tag 加 -pg 后缀）
// shared: 复用别的服务镜像（outbox publisher 与主服务同镜像，不单列变量）
const SERVICES = [
  { env: 'IMAGE_API_GATEWAY', image: 'api-gateway', dir: 'api-gateway', db: false },
  { env: 'IMAGE_TOKEN_SERVICE', image: 'token-service', dir: 'token-service', db: true },
  { env: 'IMAGE_IAM_SERVICE', image: 'iam-service', dir: 'iam-service', db: true },
  { env: 'IMAGE_NOTIFICATION_SERVICE', image: 'notification-service', dir: 'notification-service', db: true },
  { env: 'IMAGE_FILE_SERVICE', image: 'file-service', dir: 'file-service', db: true },
  { env: 'IMAGE_AUDIT_SERVICE', image: 'audit-service', dir: 'audit-service', db: true },
  { env: 'IMAGE_PRUE_BACKEND', image: 'prue-admin-backend', dir: 'prue_backend', db: true },
];

// 无独立镜像、但与主服务同版本发布的服务目录（参与版本一致性校验）。
const SHARED_DIRS = [
  { dir: 'iam-outbox-publisher', image: 'iam-service' },
];

// 方言 → tag 后缀。mysql 是历史默认（tag 无后缀）；pg 变体加 -pg。
const DIALECT_SUFFIX = { mysql: '', pg: '-pg' };

const DEFAULT_REGISTRY = 'zhcoder-docker-registry.com:8000';
const DEFAULT_REPO = 'micro-server';

function hasDialect(dialect) {
  return Object.prototype.hasOwnProperty.call(DIALECT_SUFFIX, dialect);
}

// ── 版本号归一 ───────────────────────────────────────────────────────────────
// CI 推送时用 ${CI_COMMIT_TAG#v} 剥掉 v，registry 里没有 v 前缀的 tag。
function normalizeVersion(v) {
  return String(v || '').trim().replace(/^v/, '');
}

function validateVersion(v) {
  return /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(v);
}

// ── 生成 .image.env 内容 ─────────────────────────────────────────────────────
function buildImageEnv({ version, dialect = 'pg', registry = DEFAULT_REGISTRY, repo = DEFAULT_REPO }) {
  const ver = normalizeVersion(version);
  if (!validateVersion(ver)) throw new Error(`invalid version: ${version}`);
  if (!hasDialect(dialect)) throw new Error(`invalid dialect: ${dialect} (allowed: mysql | pg)`);

  const suffix = DIALECT_SUFFIX[dialect];
  const gateway = SERVICES.find((s) => !s.db);
  const dbs = SERVICES.filter((s) => s.db);

  const lines = [];
  lines.push('# examples/docker-deploy/.image.env —— 本系统服务镜像引用（发布即固化）');
  lines.push('# ─────────────────────────────────────────────────────────────────────────────');
  lines.push('# 本文件是「本系统自建镜像」的唯一真相源：docker-compose-services.yml 的');
  lines.push('# image: 只写变量名（${IMAGE_FILE_SERVICE} 等），具体镜像名与 tag 全在这里。');
  lines.push('#');
  lines.push('# 机器生成 —— 由 `node scripts/update-image-env.js -v <ver>` 或');
  lines.push('# `coder release images` 整体重写；手工改动会在下次发布时被覆盖。');
  lines.push('# 版本号本身来自根目录 release.toml（发布单一真相源）。');
  lines.push('#');
  lines.push('# 部署时 start.sh 会加载本文件（.env 之后加载 → 镜像钉版为权威）。');
  lines.push('# 缺任何一条变量，compose 会在启动前直接报错，不会静默拉错镜像。');
  lines.push('#');
  lines.push('# 变量名 = IMAGE_ + 大写服务名；取值 = <registry>/<repo>/<image-name>:<tag>');
  lines.push('#   - image-name 与 compose service 名不一定同名：prue_backend → prue-admin-backend');
  lines.push('#   - tag 后缀：pg 方言加 -pg（1.7.0-pg）；mysql 方言无后缀；api-gateway 无 DB，两方言同 tag');
  lines.push('# ─────────────────────────────────────────────────────────────────────────────');
  lines.push('');
  lines.push('# ── 网关（无 DB，方言无关）──');
  lines.push(`${gateway.env}=${registry}/${repo}/${gateway.image}:${ver}`);
  lines.push('');
  lines.push(`# ── 业务服务（带 DB，当前为 ${dialect} 方言）──`);
  for (const s of dbs) {
    lines.push(`${s.env}=${registry}/${repo}/${s.image}:${ver}${suffix}`);
  }
  lines.push('');
  return lines.join('\n');
}

// 解析 .image.env 内容 → Map<变量名, 镜像引用>
function parseImageEnv(raw) {
  const vars = new Map();
  for (const line of String(raw || '').split('\n')) {
    const m = line.match(/^(IMAGE_[A-Z_]+)=(.*)$/);
    if (m) vars.set(m[1], m[2]);
  }
  return vars;
}

// 从 .image.env 反推版本与方言（网关无方言后缀，DB 服务带 -pg）
function deriveVersionDialect(source) {
  let raw = source;
  if (typeof source === 'string' && fs.existsSync(source)) raw = fs.readFileSync(source, 'utf8');
  if (typeof raw !== 'string') return null;
  const vars = parseImageEnv(raw);
  const gateway = vars.get('IMAGE_API_GATEWAY');
  const token = vars.get('IMAGE_TOKEN_SERVICE');
  if (!gateway || !token) return null;
  const gatewayTag = gateway.split(':').pop();
  const tokenTag = token.split(':').pop();
  if (!gatewayTag || !tokenTag) return null;
  const dialect = tokenTag.endsWith('-pg') ? 'pg' : 'mysql';
  const version = dialect === 'pg' ? tokenTag.slice(0, -'-pg'.length) : tokenTag;
  return { version, dialect };
}

// 期望的镜像引用清单（compose 展开后应与此一致）
function imageRefs({ version, dialect = 'pg', registry = DEFAULT_REGISTRY, repo = DEFAULT_REPO }) {
  const ver = normalizeVersion(version);
  return SERVICES.map((s) => {
    const suffix = s.db ? DIALECT_SUFFIX[dialect] : '';
    const tag = `${ver}${suffix}`;
    return {
      env: s.env,
      repo,
      image: s.image,
      tag,
      ref: `${registry}/${repo}/${s.image}:${tag}`,
    };
  });
}

// 写 .image.env（原子写：先写临时文件再 rename）
function writeImageEnv(file, content) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const tmp = `${file}.tmp`;
  fs.writeFileSync(tmp, content, 'utf8');
  fs.renameSync(tmp, file);
  return file;
}

module.exports = {
  ENV_FILE,
  SERVICES,
  SHARED_DIRS,
  DIALECT_SUFFIX,
  DEFAULT_REGISTRY,
  DEFAULT_REPO,
  hasDialect,
  normalizeVersion,
  validateVersion,
  buildImageEnv,
  parseImageEnv,
  deriveVersionDialect,
  imageRefs,
  writeImageEnv,
};
