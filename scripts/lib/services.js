// service 清单与「改动 → service」映射。
//
// 目录名用横杠，cargo package 名用下划线（cargo 不会自动规范化），
// 传给 `cargo -p` 时必须转换。

const { spawnSync } = require('node:child_process');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');

const SERVICE_DIRS = {
  'api-gateway': 'services/api-gateway',
  'token-service': 'services/token-service',
  'iam-service': 'services/iam-service',
  'notification-service': 'services/notification-service',
  'audit-service': 'services/audit-service',
  'file-service': 'services/file-service',
  'prue_backend': 'services/prue_backend',
};

const SERVICES = Object.keys(SERVICE_DIRS);

const SERVICE_PKG = Object.fromEntries(
  SERVICES.map((svc) => [svc, svc.replaceAll('-', '_')]),
);

// 顶层 Cargo.toml / Cargo.lock / libs/ / scripts/ 改动 → 全部 service。
const GLOBAL_PREFIXES = ['Cargo.toml', 'Cargo.lock', 'libs/', 'scripts/'];

/**
 * 从命令行参数里挑 service 名；没给就全选。
 * @param {string[]} args
 * @returns {string[]}
 */
function pickTargets(args) {
  const filtered = args.filter((s) => SERVICES.includes(s));
  return filtered.length > 0 ? filtered : SERVICES.slice();
}

/**
 * 按 git 改动挑 service。返回 null 表示 git 状态拿不到（调用方应回退到全量）。
 * @param {string[]} args
 * @param {(msg: string) => void} [log]
 * @returns {string[]|null}
 */
function pickIncrementalTargets(args, log = console.log) {
  if (!args.includes('--incremental')) return null;

  const status = spawnSync('git', ['status', '--porcelain', '--untracked-files=all'], {
    encoding: 'utf8',
    cwd: ROOT,
  });
  if (status.status !== 0) return null;

  const changedFiles = [];
  for (const line of (status.stdout || '').split('\n')) {
    if (!line) continue;
    const m = line.match(/^[ ?!]{2}\s+(.+)$/);
    if (!m) continue;
    changedFiles.push(m[1].includes(' -> ') ? m[1].split(' -> ').pop() : m[1]);
  }

  if (changedFiles.length === 0) {
    log('[incremental] no changes detected, nothing to test');
    return [];
  }

  const globalHit = changedFiles.some((f) =>
    GLOBAL_PREFIXES.some((p) => f === p.slice(0, -1) || f.startsWith(p)),
  );
  if (globalHit) {
    log('[incremental] global change detected, running all services');
    return SERVICES.slice();
  }

  const targets = [];
  for (const [svc, dir] of Object.entries(SERVICE_DIRS)) {
    if (changedFiles.some((f) => f === dir || f.startsWith(dir + '/'))) targets.push(svc);
  }
  if (targets.length === 0) {
    log('[incremental] no service touched, nothing to test');
    return [];
  }
  log('[incremental] changed services:', targets.join(', '));
  return targets;
}

module.exports = {
  ROOT,
  SERVICE_DIRS,
  SERVICE_PKG,
  SERVICES,
  pickTargets,
  pickIncrementalTargets,
};
