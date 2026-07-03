// 在 docker 容器中跑各 service 的 cargo 单元测试（volume mapping 模型）。
// 源码、target、cargo registry 全部用挂载，避免 rebuild 镜像，增量编译跨 service 复用。
// 用法：
//   node scripts/run-tests-docker.js                 # 跑全部 services
//   node scripts/run-tests-docker.js iam-service     # 跑指定 service（可多个）
const { spawnSync } = require('node:child_process');
const path = require('node:path');

const ROOT = path.join(__dirname, '..');

const IMAGE = 'zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl';
const TARGET = 'x86_64-unknown-linux-musl';

const SERVICES = [
  'api-gateway',
  'token-service',
  'iam-service',
  'notification-service',
  'audit-service',
  'config-service',
  'file-service',
  'prue_backend',
];

const CONCURRENCY = 5;

function pickTargets(args) {
  const filtered = args.filter(s => SERVICES.includes(s));
  return filtered.length > 0 ? filtered : SERVICES;
}

function run(cmd, args, opts = {}) {
  const res = spawnSync(cmd, args, { stdio: 'inherit', cwd: ROOT, ...opts });
  return res.status === 0;
}

// 列出当前所有 mysql:* / testcontainers ryuk 的容器 id（运行中 + 已退出）。
// 用于在测试前后做差分，仅清理本次新生出来的 testcontainers 容器，避免误伤宿主上其它 mysql。
function listTestContainerIds() {
  const ids = new Set();
  const filters = [
    ['--filter', 'ancestor=mysql:8.0'],
    ['--filter', 'ancestor=mysql:8.1'],
    ['--filter', 'ancestor=mysql:8'],
    ['--filter', 'ancestor=mysql:latest'],
    ['--filter', 'name=testcontainers'],
    ['--filter', 'label=org.testcontainers'],
  ];
  for (const f of filters) {
    const res = spawnSync('docker', ['ps', '-aq', ...f], { encoding: 'utf8' });
    if (res.status === 0 && res.stdout) {
      for (const id of res.stdout.trim().split('\n').filter(Boolean)) ids.add(id);
    }
  }
  return ids;
}

function cleanupNewContainers(beforeIds) {
  const after = listTestContainerIds();
  const newOnes = [...after].filter(id => !beforeIds.has(id));
  if (newOnes.length === 0) return;
  console.log(`\n[cleanup] removing ${newOnes.length} test mysql container(s)...`);
  spawnSync('docker', ['rm', '-f', ...newOnes], { stdio: 'inherit' });
}

function runTests(svc) {
  console.log(`\n[test] ${svc}`);
  return run('docker', [
    'run', '--rm',
    '--network', 'host',
    '-v', `${ROOT}:/app`,
    '-v', 'coder-cargo-registry:/usr/local/cargo/registry',
    '-v', 'coder-cargo-git:/usr/local/cargo/git',
    '-v', 'coder-cargo-target:/app/target',
    '-v', '/var/run/docker.sock:/var/run/docker.sock',
    '-w', '/app',
    IMAGE,
    'cargo', 'test', '--target', TARGET, '-p', svc, '--', '--nocapture',
  ]);
}

function main() {
  const targets = pickTargets(process.argv.slice(2));
  const before = listTestContainerIds();

  // 包装一次清理，确保 Ctrl-C / 异常退出也能跑到
  let cleaned = false;
  const doCleanup = () => {
    if (cleaned) return;
    cleaned = true;
    try { cleanupNewContainers(before); } catch (e) { console.error('[cleanup] failed:', e); }
  };
  process.on('SIGINT',  () => { doCleanup(); process.exit(130); });
  process.on('SIGTERM', () => { doCleanup(); process.exit(143); });
  process.on('exit', doCleanup);

  const failed = [];
  try {
    for (let i = 0; i < targets.length; i += CONCURRENCY) {
      const batch = targets.slice(i, i + CONCURRENCY);
      const results = batch.map(svc => ({ svc, ok: runTests(svc) }));
      for (const { svc, ok } of results) {
        if (!ok) failed.push(`${svc} (test)`);
      }
    }
  } finally {
    doCleanup();
  }
  console.log('\n=== summary ===');
  console.log(`total : ${targets.length}`);
  console.log(`pass  : ${targets.length - failed.length}`);
  console.log(`fail  : ${failed.length}`);
  for (const f of failed) console.log(`  - ${f}`);
  process.exit(failed.length ? 1 : 0);
}

main();
