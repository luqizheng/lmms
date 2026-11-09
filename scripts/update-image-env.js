#!/usr/bin/env node
/**
 * update-image-env.js —— 发布镜像后更新部署单元的 .image.env（镜像钉版）
 *
 * 背景（见 docs/superpowers/specs/2026-11-09-image-env-driven-deploy-design.md）：
 *   compose 的 image: 只写整名变量（${IMAGE_FILE_SERVICE}），值集中在
 *   examples/docker-deploy/.image.env。发布新镜像后跑一次本脚本即可把
 *   部署单元的镜像版本推进到新版本，且版本变化在 git diff 里看得见。
 *
 * 用法：
 *   node scripts/update-image-env.js -v 1.7.0 --dialect pg     # 写入 .image.env
 *   node scripts/update-image-env.js -v 1.7.0 --check           # 只比对，不写（CI 门禁）
 *   node scripts/update-image-env.js --check                    # 自洽校验（从现文件反推版本）
 *   node scripts/update-image-env.js -v 1.7.0 --verify          # 确认镜像在 registry 里
 *   node scripts/update-image-env.js -v 1.7.0 --dry-run         # 打印将写入的内容
 *
 * 退出码：0 成功；1 漂移/校验失败；2 参数错误
 */

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const https = require('node:https');

const REPO_ROOT = path.join(__dirname, '..');

const DEFAULT_REGISTRY = 'zhcoder-docker-registry.com:8000';
const DEFAULT_REPO = 'micro-server';
const DEFAULT_DIR = path.join('examples', 'docker-deploy');
const ENV_FILE = '.image.env';

// ── 服务 → 镜像映射 ──────────────────────────────────────────────────────────
// env   : .image.env 里的变量名（compose 的 image: 引用它）
// image : registry 里的镜像名（≠ compose service 名时在此显式映射）
// db    : 是否带 DB（带 DB 的服务有 pg 方言变体，tag 加 -pg 后缀）
const SERVICES = [
  { env: 'IMAGE_API_GATEWAY', image: 'api-gateway', db: false },
  { env: 'IMAGE_TOKEN_SERVICE', image: 'token-service', db: true },
  { env: 'IMAGE_IAM_SERVICE', image: 'iam-service', db: true },
  { env: 'IMAGE_NOTIFICATION_SERVICE', image: 'notification-service', db: true },
  { env: 'IMAGE_FILE_SERVICE', image: 'file-service', db: true },
  { env: 'IMAGE_AUDIT_SERVICE', image: 'audit-service', db: true },
  { env: 'IMAGE_PRUE_BACKEND', image: 'prue-admin-backend', db: true },
];

// 方言 → tag 后缀。mysql 是历史默认（tag 无后缀）；pg 变体加 -pg。
const DIALECT_SUFFIX = { mysql: '', pg: '-pg' };

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
  lines.push('# 机器生成 —— 由 `node scripts/update-image-env.js -v <ver>` 整体重写；');
  lines.push('# 手工改动会在下次发布时被覆盖。版本升级流程：');
  lines.push('#   1. 发布镜像：node packages/cli/bin/coder.js build images -v <ver> --push');
  lines.push('#                node packages/cli/bin/coder.js build images -v <ver> --dialect pg --push');
  lines.push('#   2. 更新本文件：node scripts/update-image-env.js -v <ver> --dialect pg');
  lines.push('#   3. 提交 .image.env 的 diff（版本变化在 git 里看得见）');
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

// 从现有 .image.env 反推版本与方言（--check 未给 -v 时用）
function deriveVersionDialect(file) {
  if (!fs.existsSync(file)) return null;
  const raw = fs.readFileSync(file, 'utf8');
  const vars = new Map();
  for (const line of raw.split('\n')) {
    const m = line.match(/^(IMAGE_[A-Z_]+)=(.*)$/);
    if (m) vars.set(m[1], m[2]);
  }
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

// ── 参数解析 ────────────────────────────────────────────────────────────────
function parseArgs(argv) {
  const args = {
    version: null,
    dialect: 'pg',
    registry: DEFAULT_REGISTRY,
    repo: DEFAULT_REPO,
    dir: DEFAULT_DIR,
    check: false,
    verify: false,
    dryRun: false,
    help: false,
    unknown: [],
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case '-h': case '--help': args.help = true; break;
      case '-v': case '--version': args.version = argv[++i] || null; break;
      case '--dialect': args.dialect = String(argv[++i] || '').toLowerCase(); break;
      case '--registry': args.registry = argv[++i] || DEFAULT_REGISTRY; break;
      case '--repo': args.repo = argv[++i] || DEFAULT_REPO; break;
      case '--dir': args.dir = argv[++i] || DEFAULT_DIR; break;
      case '--check': args.check = true; break;
      case '--verify': args.verify = true; break;
      case '--dry-run': args.dryRun = true; break;
      default:
        if (a.startsWith('--version=')) { args.version = a.slice('--version='.length); break; }
        if (a.startsWith('--dialect=')) { args.dialect = a.slice('--dialect='.length).toLowerCase(); break; }
        if (a.startsWith('--registry=')) { args.registry = a.slice('--registry='.length); break; }
        if (a.startsWith('--repo=')) { args.repo = a.slice('--repo='.length); break; }
        if (a.startsWith('--dir=')) { args.dir = a.slice('--dir='.length); break; }
        args.unknown.push(a);
    }
  }
  return args;
}

const HELP = [
  'usage: node scripts/update-image-env.js -v <version> [options]',
  '',
  'options:',
  '  -v, --version <ver>   镜像版本（自动剥前导 v；必填）',
  '      --dialect <d>    pg（默认）| mysql —— 决定 tag 后缀（-pg / 无）',
  '      --registry <h>   镜像仓库主机（默认 ' + DEFAULT_REGISTRY + '）',
  '      --repo <p>       镜像仓库前缀（默认 ' + DEFAULT_REPO + '）',
  '      --dir <path>     部署单元目录（默认 ' + DEFAULT_DIR + '）',
  '      --check          只比对「期望 vs 现文件」，有漂移则 exit 1',
  '                       （不给 -v 时从现文件反推版本/方言做自洽性校验）',
  '      --verify         调 registry v2 API 确认镜像存在',
  '      --dry-run        打印将写入的内容，不落盘',
  '  -h, --help            显示帮助',
];

// ── registry v2 manifest 探测（可选） ───────────────────────────────────────
const MANIFEST_ACCEPT = [
  'application/vnd.docker.distribution.manifest.v2+json',
  'application/vnd.docker.distribution.manifest.list.v2+json',
  'application/vnd.oci.image.manifest.v1+json',
  'application/vnd.oci.image.index.v1+json',
].join(', ');

function request(urlStr, { method = 'GET', headers = {}, timeoutMs = 15000 } = {}) {
  return new Promise((resolve) => {
    const u = new URL(urlStr);
    const mod = u.protocol === 'http:' ? http : https;
    const req = mod.request(
      u,
      { method, headers: { accept: MANIFEST_ACCEPT, ...headers }, timeout: timeoutMs },
      (res) => {
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () =>
          resolve({
            status: res.statusCode,
            headers: res.headers,
            body: Buffer.concat(chunks).toString('utf8'),
          })
        );
      }
    );
    req.on('timeout', () => { req.destroy(new Error('timeout')); });
    req.on('error', (err) => resolve({ status: 0, headers: {}, body: '', error: err.message }));
    req.end();
  });
}

async function fetchToken(realm, service, scope, timeoutMs) {
  const u = new URL(realm);
  if (service) u.searchParams.set('service', service);
  if (scope) u.searchParams.set('scope', scope);
  const res = await request(u.toString(), { headers: { accept: 'application/json' }, timeoutMs });
  if (res.status !== 200) return null;
  try {
    const data = JSON.parse(res.body);
    return data.token || data.access_token || null;
  } catch {
    return null;
  }
}

// 探一次 manifest：https 优先，失败再试 http（内网 registry 常见明文端口）。
async function probeManifest(registry, repo, image, tag, { timeoutMs = 15000 } = {}) {
  const base = `${registry}/${repo}/${image}/manifests/${tag}`;
  const candidates = base.startsWith('http://') || base.startsWith('https://')
    ? [base]
    : [`https://${base}`, `http://${base}`];

  let last = null;
  for (const url of candidates) {
    let res = await request(url, { method: 'HEAD', timeoutMs });
    if (res.status === 0) {
      last = last || { ok: false, reason: res.error || 'request failed' };
      continue;
    }
    if (res.status === 200) return { ok: true, status: 200 };

    // Bearer 认证挑战
    const wwwAuth = res.headers['www-authenticate'] || '';
    if (res.status === 401 && /^bearer/i.test(wwwAuth)) {
      const realm = (wwwAuth.match(/realm="([^"]+)"/) || [])[1];
      const service = (wwwAuth.match(/service="([^"]+)"/) || [])[1];
      const scope = (wwwAuth.match(/scope="([^"]+)"/) || [])[1]
        || `repository:${repo}/${image}:pull`;
      if (realm) {
        const token = await fetchToken(realm, service, scope, timeoutMs);
        if (token) {
          res = await request(url, { method: 'HEAD', headers: { authorization: `Bearer ${token}` }, timeoutMs });
          if (res.status === 200) return { ok: true, status: 200 };
          last = { ok: false, reason: `HTTP ${res.status}` };
          continue;
        }
      }
    }
    last = { ok: false, reason: `HTTP ${res.status}` };
  }
  return last || { ok: false, reason: 'unknown' };
}

// ── 主流程 ──────────────────────────────────────────────────────────────────
async function main(argv) {
  const args = parseArgs(argv);
  if (args.help) {
    console.log(HELP.join('\n'));
    return 0;
  }
  if (args.unknown.length) {
    console.error(`[update-image-env] 未知参数: ${args.unknown.join(', ')}`);
    console.error(HELP.join('\n'));
    return 2;
  }
  if (!args.version && !(args.check && !args.dryRun)) {
    console.error('[update-image-env] FATAL: 缺少 -v <version>');
    console.error(HELP.join('\n'));
    return 2;
  }

  const dir = path.isAbsolute(args.dir) ? args.dir : path.join(REPO_ROOT, args.dir);
  const target = path.join(dir, ENV_FILE);

  // 版本与方言：给了 -v 就用它；--check 没给 -v 则从现文件反推（自洽性校验）
  let version = normalizeVersion(args.version);
  let dialect = args.dialect;
  if (!args.version) {
    const derived = deriveVersionDialect(target);
    if (!derived) {
      console.error(`[update-image-env] ✗ ${target} 不存在或无法解析版本/方言`);
      console.error('  运行 node scripts/update-image-env.js -v <version> 生成');
      return 1;
    }
    version = derived.version;
    dialect = derived.dialect;
    console.log(`[update-image-env] 未给 -v，从 ${target} 反推 version=${version} dialect=${dialect}`);
  }

  if (!hasDialect(dialect)) {
    console.error(`[update-image-env] FATAL: 非法 dialect: ${dialect}（允许 mysql | pg）`);
    return 2;
  }
  if (!validateVersion(version)) {
    console.error(`[update-image-env] FATAL: 非法版本号: ${args.version}`);
    return 2;
  }

  const content = buildImageEnv({
    version,
    dialect,
    registry: args.registry,
    repo: args.repo,
  });

  // 期望的镜像引用（供 --verify 用）
  const images = SERVICES.map((s) => {
    const suffix = s.db ? DIALECT_SUFFIX[dialect] : '';
    const tag = `${version}${suffix}`;
    return {
      env: s.env,
      repo: args.repo,
      image: s.image,
      tag,
      ref: `${args.registry}/${args.repo}/${s.image}:${tag}`,
    };
  });

  if (args.dryRun) {
    console.log(content);
    return 0;
  }

  if (args.check) {
    const current = fs.existsSync(target) ? fs.readFileSync(target, 'utf8') : '';
    if (current === content) {
      console.log(`[update-image-env] ✓ ${target} 与 -v ${version} --dialect ${dialect} 一致`);
    } else {
      console.error(`[update-image-env] ✗ ${target} 与 -v ${version} --dialect ${dialect} 不一致`);
      if (!fs.existsSync(target)) {
        console.error('  文件不存在 —— 运行 node scripts/update-image-env.js -v ' + version);
      } else {
        const cur = new Map(
          current.split('\n').filter((l) => l.startsWith('IMAGE_')).map((l) => [l.split('=')[0], l.split('=').slice(1).join('=')])
        );
        for (const img of images) {
          const now = cur.get(img.env);
          if (now !== img.ref) console.error(`  ${img.env}: 现值 ${now || '(缺失)'} → 应为 ${img.ref}`);
        }
      }
      return 1;
    }
  } else {
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(target, content, 'utf8');
    console.log(`[update-image-env] ✓ 已写入 ${target}（version=${version}, dialect=${dialect}）`);
    for (const img of images) console.log(`  ${img.env}=${img.ref}`);
  }

  if (args.verify) {
    console.log(`[update-image-env] 校验 registry ${args.registry} …`);
    let failed = 0;
    for (const img of images) {
      const r = await probeManifest(args.registry, img.repo, img.image, img.tag);
      if (r.ok) {
        console.log(`  ✓ ${img.ref}`);
      } else {
        failed++;
        console.error(`  ✗ ${img.ref} —— ${r.reason}`);
      }
    }
    if (failed) {
      console.error(`[update-image-env] ✗ ${failed} 个镜像在 registry 中不存在`);
      return 1;
    }
  }

  return 0;
}

if (require.main === module) {
  main(process.argv.slice(2))
    .then((code) => process.exit(code))
    .catch((err) => {
      console.error(`[update-image-env] FATAL: ${err.message}`);
      process.exit(1);
    });
}

module.exports = {
  SERVICES,
  DIALECT_SUFFIX,
  DEFAULT_REGISTRY,
  DEFAULT_REPO,
  DEFAULT_DIR,
  ENV_FILE,
  normalizeVersion,
  validateVersion,
  buildImageEnv,
  parseArgs,
  probeManifest,
  deriveVersionDialect,
};
