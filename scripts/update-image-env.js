#!/usr/bin/env node
/**
 * update-image-env.js —— 更新部署单元的 .image.env（镜像钉版）
 *
 * 版本号的真相源是根目录 release.toml；本脚本是它的独立入口
 * （CI 门禁 `deploy_image_env_check` 调它，也可在没有 CLI 的环境里用）。
 *
 * 用法：
 *   node scripts/update-image-env.js -v 1.7.1 --dialect pg     # 写入 .image.env
 *   node scripts/update-image-env.js -v 1.7.1 --check           # 只比对，不写（CI 门禁）
 *   node scripts/update-image-env.js --check                    # 自洽校验（从现文件反推版本）
 *   node scripts/update-image-env.js -v 1.7.1 --verify          # 确认镜像在 registry 里
 *   node scripts/update-image-env.js -v 1.7.1 --dry-run         # 打印将写入的内容
 *
 * 退出码：0 成功；1 漂移/校验失败；2 参数错误
 */

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const https = require('node:https');

const imageEnv = require('../packages/cli/lib/image-env.js');
const { readReleaseConfig } = require('../packages/cli/lib/release-toml.js');
const { readServiceVersions } = require('../packages/cli/lib/service-versions.js');

const REPO_ROOT = path.join(__dirname, '..');

const {
  ENV_FILE,
  SERVICES,
  BASE_IMAGES,
  DEPLOY_UNITS,
  DIALECT_SUFFIX,
  DEFAULT_REGISTRY,
  DEFAULT_REPO,
  hasDialect,
  normalizeVersion,
  validateVersion,
  buildImageEnv,
  deriveVersionDialect,
  imageRefs,
  writeImageEnv,
} = imageEnv;

// ── 参数解析 ────────────────────────────────────────────────────────────────
function parseArgs(argv) {
  const args = {
    version: null,
    dialect: null, // null = 跟随 release.toml
    registry: null,
    repo: null,
    dir: null, // null = 全部部署单元（DEPLOY_UNITS）
    scheme: 'auto',
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
      case '--registry': args.registry = argv[++i] || null; break;
      case '--repo': args.repo = argv[++i] || null; break;
      case '--dir': args.dir = argv[++i] || null; break;
      case '--scheme': args.scheme = String(argv[++i] || '').toLowerCase(); break;
      case '--check': args.check = true; break;
      case '--verify': args.verify = true; break;
      case '--dry-run': args.dryRun = true; break;
      default:
        if (a.startsWith('--version=')) { args.version = a.slice('--version='.length); break; }
        if (a.startsWith('--dialect=')) { args.dialect = a.slice('--dialect='.length).toLowerCase(); break; }
        if (a.startsWith('--registry=')) { args.registry = a.slice('--registry='.length); break; }
        if (a.startsWith('--repo=')) { args.repo = a.slice('--repo='.length); break; }
        if (a.startsWith('--dir=')) { args.dir = a.slice('--dir='.length); break; }
        if (a.startsWith('--scheme=')) { args.scheme = a.slice('--scheme='.length).toLowerCase(); break; }
        args.unknown.push(a);
    }
  }
  return args;
}

const HELP = [
  'usage: node scripts/update-image-env.js [-v <version>] [options]',
  '',
  'options:',
  '  -v, --version <ver>   镜像版本（自动剥前导 v；不给则读 services/* 的 crate 版本）',
  '      --dialect <d>    pg | mysql —— 决定 tag 后缀（不给则读 release.toml）',
  '      --registry <h>   镜像仓库主机（不给则读 release.toml）',
  '      --repo <p>       镜像仓库前缀（不给则读 release.toml）',
  '      --dir <path>     只处理指定部署单元（默认：全部 DEPLOY_UNITS）',
  '      --scheme <s>    registry 协议：http | https | auto（默认 auto=http 优先）',
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

// 200 响应的 content-type 必须是 manifest 类，才算「镜像存在」。
// 原因：对明文端口发一次 TLS ClientHello 会让 nginx 退回 default server，
// 之后同一 host:port 的 HTTP 请求恒返回 200 + text/html 默认页 ——
// 不看 content-type 就会把「任何 tag」都判成存在。
const MANIFEST_CONTENT_TYPES = [
  'application/vnd.docker.distribution.manifest.v2+json',
  'application/vnd.docker.distribution.manifest.list.v2+json',
  'application/vnd.oci.image.manifest.v1+json',
  'application/vnd.oci.image.index.v1+json',
];

function looksLikeManifest(res) {
  const ct = String((res && res.headers && res.headers['content-type']) || '').toLowerCase();
  if (MANIFEST_CONTENT_TYPES.some((t) => ct.includes(t))) return true;
  // 兜底：老实现可能不带 content-type，但会带 docker-content-digest
  return Boolean(res && res.headers && res.headers['docker-content-digest']);
}

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

// 协议候选：http 优先（内网 registry 多为明文端口），连不上 / 被重定向再试 https。
// ⚠ 不可写成 https 优先：对明文端口发 TLS ClientHello 会把 nginx 打成 default
//   server 模式，后续同 host:port 的 HTTP 恒回 200 + text/html（见 looksLikeManifest）。
function schemeCandidates(registry, scheme) {
  const base = String(registry).replace(/^https?:\/\//, '');
  if (scheme === 'http') return [`http://${base}`];
  if (scheme === 'https') return [`https://${base}`];
  return [`http://${base}`, `https://${base}`];
}

// 换协议的条件：连不上（0）/ 重定向（3xx）/ 400（明文打到 TLS 端口）
function shouldTryNextScheme(status) {
  return status === 0 || status === 400 || (status >= 300 && status < 400);
}

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

// 单次探测（含 Bearer 认证挑战处理）
// transient=true 表示「响应本身可疑但非终局」（200 却不是 manifest）——可重试。
async function attemptManifest(url, { repo, image, timeoutMs }) {
  let res = await request(url, { method: 'HEAD', timeoutMs });
  const notManifest = (r) => ({
    ok: false,
    transient: true,
    reason: `HTTP 200 但 content-type 非 manifest（${r.headers['content-type'] || '无'}）`,
  });

  if (res.status === 200) {
    return looksLikeManifest(res) ? { ok: true, status: 200 } : notManifest(res);
  }

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
        if (res.status === 200) {
          return looksLikeManifest(res) ? { ok: true, status: 200 } : notManifest(res);
        }
        return { ok: false, transient: false, reason: `HTTP ${res.status}`, status: res.status };
      }
    }
  }
  return { ok: false, transient: false, reason: `HTTP ${res.status}`, status: res.status };
}

// 探一次 manifest：协议候选 × 瞬时态重试
async function probeManifest(registry, repo, image, tag, { timeoutMs = 15000, scheme = 'auto', retries = 2 } = {}) {
  // Docker Registry v2 API：/v2/<name>/manifests/<tag>。
  // ⚠ 漏掉 /v2/ 前缀时 nginx 会落到门户默认站点 —— 恒回 200 + text/html。
  const path = `v2/${repo}/${image}/manifests/${tag}`;
  let last = null;

  for (const base of schemeCandidates(registry, scheme)) {
    const url = `${base}/${path}`;
    for (let attempt = 0; attempt <= retries; attempt++) {
      const res = await attemptManifest(url, { repo, image, timeoutMs });
      if (res.ok) return res;

      if (res.status !== undefined && shouldTryNextScheme(res.status) && scheme === 'auto') {
        last = last || res;
        break; // 换协议
      }
      last = res;
      if (!res.transient || attempt === retries) break;
      await sleep(500 * (attempt + 1)); // 瞬时态（nginx default server 态）退避重试
    }
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

  // 版本真相源：services/*/Cargo.toml（根 [workspace.package].version 继承）；
  // 镜像参数（方言 / 镜像源）：release.toml。命令行显式给的参数优先。
  let release = null;
  try {
    release = readReleaseConfig(REPO_ROOT);
  } catch {
    release = null; // release.toml 缺失时退化为「全靠命令行 + 服务版本」
  }

  let version = normalizeVersion(args.version);
  let dialect = args.dialect || (release ? release.dialect : 'pg');
  const registry = args.registry || (release ? release.registry : DEFAULT_REGISTRY);
  const repo = args.repo || (release ? release.repo : DEFAULT_REPO);

  if (!args.version) {
    // 期望版本一律取服务 crate 版本 —— 这是发布门禁的关键：
    // `--check` 若改成「从现文件反推」，就永远发现不了「改了 Cargo.toml 版本
    // 但没更新 .image.env」这类真实漂移。
    const versions = readServiceVersions(REPO_ROOT);
    if (versions.missing.length) {
      console.error(`[update-image-env] FATAL: 服务 Cargo.toml 缺失：${versions.missing.map((m) => m.dir).join(', ')}`);
      return 2;
    }
    if (versions.splits.length) {
      console.error(`[update-image-env] FATAL: 服务版本不一致（期望 ${versions.version}）：` +
        versions.splits.map((s) => `${s.dir}=${s.version}`).join(', '));
      return 2;
    }
    version = versions.version;
    console.log(`[update-image-env] 未给 -v，取服务 crate 版本 version=${version}`);
  }

  if (!hasDialect(dialect)) {
    console.error(`[update-image-env] FATAL: 非法 dialect: ${dialect}（允许 mysql | pg）`);
    return 2;
  }
  if (!validateVersion(version)) {
    console.error(`[update-image-env] FATAL: 非法版本号: ${args.version}`);
    return 2;
  }

  const refs = imageRefs({ version, dialect, registry, repo });

  // 目标部署单元：--dir 指定单个；否则全部（services-only + 全栈）
  const units = args.dir
    ? [{ dir: path.isAbsolute(args.dir) ? args.dir : path.join(REPO_ROOT, args.dir), base: null }]
    : DEPLOY_UNITS.map((u) => ({ dir: path.join(REPO_ROOT, u.dir), base: u.base }));

  if (args.dryRun) {
    for (const u of units) {
      const target = path.join(u.dir, ENV_FILE);
      const existingRaw = fs.existsSync(target) ? fs.readFileSync(target, 'utf8') : '';
      const includeBase = u.base !== null ? u.base : existingRaw.includes('IMAGE_CONSUL=');
      console.log(`# ==== ${path.relative(REPO_ROOT, target)} ====`);
      console.log(buildImageEnv({ version, dialect, registry, repo, includeBase, existingRaw }));
    }
    return 0;
  }

  let failures = 0;
  const written = [];
  for (const u of units) {
    const target = path.join(u.dir, ENV_FILE);
    const existingRaw = fs.existsSync(target) ? fs.readFileSync(target, 'utf8') : '';
    const includeBase = u.base !== null ? u.base : existingRaw.includes('IMAGE_CONSUL=');
    const content = buildImageEnv({ version, dialect, registry, repo, includeBase, existingRaw });

    if (!args.check) {
      writeImageEnv(target, content);
      written.push(target);
      continue;
    }

    // ── --check：平台镜像段必须完全一致；基础镜像段只要求变量齐全（允许手工升版）
    const cur = imageEnv.parseImageEnv(existingRaw);
    if (!fs.existsSync(target)) {
      console.error(`[update-image-env] ✗ ${target} 不存在 —— 运行 node scripts/update-image-env.js`);
      failures++;
      continue;
    }
    let drift = 0;
    for (const img of refs) {
      const now = cur.get(img.env);
      if (now !== img.ref) {
        console.error(`  ✗ ${path.relative(REPO_ROOT, target)} ${img.env}: 现值 ${now || '(缺失)'} → 应为 ${img.ref}`);
        drift++;
      }
    }
    if (includeBase) {
      for (const b of BASE_IMAGES) {
        if (!cur.get(b.env)) {
          console.error(`  ✗ ${path.relative(REPO_ROOT, target)} ${b.env}: 缺失（基础镜像）`);
          drift++;
        }
      }
    }
    if (drift) failures++;
    else console.log(`[update-image-env] ✓ ${path.relative(REPO_ROOT, target)} 与 version=${version} dialect=${dialect} 一致`);
  }

  if (!args.check && written.length) {
    console.log(`[update-image-env] ✓ 已写入 ${written.length} 个部署单元（version=${version}, dialect=${dialect}）`);
    for (const f of written) console.log(`  → ${path.relative(REPO_ROOT, f)}`);
    for (const img of refs) console.log(`    ${img.env}=${img.ref}`);
  }

  if (failures) return 1;

  if (args.verify) {
    console.log(`[update-image-env] 校验 registry ${registry} …`);
    let failed = 0;
    for (const img of refs) {
      const r = await probeManifest(registry, img.repo, img.image, img.tag, { scheme: args.scheme });
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
  ENV_FILE,
  normalizeVersion,
  validateVersion,
  buildImageEnv,
  parseArgs,
  probeManifest,
  deriveVersionDialect,
  looksLikeManifest,
  schemeCandidates,
};
