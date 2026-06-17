const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const ROOT_DIR = path.join(__dirname, '..');
const SERVICES_DIR = path.join(ROOT_DIR, 'services');
const REGISTRY_HOST = 'zhcoder-docker-registry.com:8000';

const SERVICES = [
  'api-gateway',
  'token-service',
  'iam-service',
  'notification-service',
  'file-service',
  'config-service',
  'audit-service',
  'prue_backend',
];

function listTargets(args) {
  const filtered = args.filter(s => SERVICES.includes(s));
  return filtered.length > 0 ? filtered : SERVICES;
}

function buildService(name) {
  const dockerfile = path.join(SERVICES_DIR, name, 'Dockerfile');
  if (!fs.existsSync(dockerfile)) {
    console.log(`[skip] ${name} 无 Dockerfile`);
    return false;
  }
  const image = `${REGISTRY_HOST}/micro-server/${name}`;
  const cmd = [
    'docker', 'buildx', 'build', '--load',
    '-f', path.relative(ROOT_DIR, dockerfile),
    '-t', image,
    '.',
  ];
  console.log(`\n[build] ${name} -> ${image}`);
  const res = spawnSync(cmd[0], cmd.slice(1), {
    cwd: ROOT_DIR,
    stdio: 'inherit',
    shell: false,
  });
  if (res.status !== 0) {
    console.log(`[fail] ${name}`);
    return false;
  }
  console.log(`[ok]   ${name}`);
  return true;
}

function pruneCache() {
  console.log('[clean] 清理 buildx cache（cargo-registry / cargo-git）');
  for (const id of ['cargo-registry', 'cargo-git']) {
    const res = spawnSync('docker', ['buildx', 'prune', '--force', '--cache-filter', `id==${id}`], {
      stdio: 'inherit',
      shell: false,
    });
    if (res.status !== 0) {
      console.log(`[warn] prune ${id} 失败`);
    }
  }
}

function main() {
  const argv = process.argv.slice(2);
  const cmd = argv[0];

  if (cmd === 'clean' || cmd === 'prune') {
    pruneCache();
    return;
  }

  const targets = listTargets(argv);
  if (targets.length === 0) {
    console.log(`[error] 无可用服务。有效值：${SERVICES.join(', ')}`);
    process.exit(2);
  }

  const start = Date.now();
  let ok = 0;
  let fail = 0;
  for (const name of targets) {
    if (buildService(name)) ok += 1; else fail += 1;
  }
  const sec = ((Date.now() - start) / 1000).toFixed(1);
  console.log(`\n[done] 成功 ${ok} 失败 ${fail} 耗时 ${sec}s`);
  if (fail > 0) process.exit(1);
}

main();