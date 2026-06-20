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

function pickTargets(args) {
  const filtered = args.filter(s => SERVICES.includes(s));
  return filtered.length > 0 ? filtered : SERVICES;
}

function run(cmd, args, opts = {}) {
  const res = spawnSync(cmd, args, { stdio: 'inherit', cwd: ROOT, ...opts });
  return res.status === 0;
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
  const failed = [];
  for (const svc of targets) {
    if (!runTests(svc)) { failed.push(`${svc} (test)`); }
  }
  console.log('\n=== summary ===');
  console.log(`total : ${targets.length}`);
  console.log(`pass  : ${targets.length - failed.length}`);
  console.log(`fail  : ${failed.length}`);
  for (const f of failed) console.log(`  - ${f}`);
  process.exit(failed.length ? 1 : 0);
}

main();
