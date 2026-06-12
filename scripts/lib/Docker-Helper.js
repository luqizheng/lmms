// Docker-Helper.js
// 封装 docker compose 与 healthcheck 等待
const { spawn, spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const { log, ROOT } = require('./utils.js');

const COMPOSE_FILE = path.join(ROOT, 'infrastructure', 'docker-compose.test-infra.yaml');

function composeArgs(...extra) {
  return ['compose', '-f', COMPOSE_FILE, ...extra];
}

const PRE_PULL_IMAGES = [
  'zhcoder-docker-registry.com:8000/official/alpine:3.21',
  'zhcoder-docker-registry.com:8000/official/rust:1.96-slim',
  'zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl',
  'zhcoder-docker-registry.com:8000/official/mysql:8.0',
  'zhcoder-docker-registry.com:8000/official/redis:7-alpine',
  'zhcoder-docker-registry.com:8000/official/consul:1.15',
];

async function prePull() {
  for (const img of PRE_PULL_IMAGES) {
    log.cyan(`[pull] ${img}`);
    const res = spawnSync('docker', ['pull', img], { stdio: 'inherit' });
    if (res.status !== 0) {
      log.yellow(`[pull] WARNING: failed to pull ${img} (will retry during build)`);
    }
  }
}

async function runCompose(args) {
  return new Promise((resolve) => {
    const p = spawn('docker', ['compose', '-f', COMPOSE_FILE, ...args], { stdio: 'inherit' });
    p.on('close', (code) => resolve(code ?? 1));
  });
}

async function startTestStack() {
  log.cyan('[docker] pre-pulling base images...');
  await prePull();
  log.cyan('[docker] starting test stack (background)...');
  const logPath = path.join(ROOT, 'reports', 'docker-up.log');
  fs.mkdirSync(path.dirname(logPath), { recursive: true });
  const out = fs.openSync(logPath, 'w');
  const err = fs.openSync(logPath + '.err', 'w');
  const p = spawn('docker', composeArgs('up', '-d'), {
    detached: true,
    stdio: ['ignore', out, err],
  });
  p.unref();
  log.gray(`[docker] PID=${p.pid}, log: ${logPath}`);
  try {
    if (!(await waitAllHealthy(1800))) {
      if (fs.existsSync(logPath + '.err')) {
        const lines = fs.readFileSync(logPath + '.err', 'utf8').split(/\r?\n/).slice(-30);
        for (const l of lines) if (l) process.stderr.write(l + '\n');
      }
      throw new Error('test stack did not become healthy');
    }
  } finally {
    try { fs.closeSync(out); } catch {}
    try { fs.closeSync(err); } catch {}
    try { fs.unlinkSync(logPath + '.err'); } catch {}
  }
}

async function stopTestStack() {
  log.cyan('[docker] stopping test stack...');
  const code = await runCompose(['down', '-v']);
  if (code !== 0) throw new Error(`docker compose down failed (exit=${code})`);
}

async function restartServices(services) {
  log.cyan(`[docker] restarting: ${services.join(', ')}`);
  await runCompose(['restart', ...services]);
  await waitAllHealthy();
}

async function getContainerName(service) {
  const tryNames = async (args) => new Promise((resolve) => {
    const p = spawn('docker', args);
    let buf = '';
    p.stdout.on('data', (d) => (buf += d.toString()));
    p.on('close', () => resolve(buf.trim()));
  });

  let id = await tryNames(composeArgs('ps', '-q', service));
  if (id) return id.split(/\r?\n/)[0].trim();
  id = await tryNames(['ps', '-a', '--filter', `label=com.docker.compose.service=${service}`, '--format', '{{.ID}}']);
  return id.split(/\r?\n/)[0].trim();
}

async function resetDatabase() {
  log.cyan('[db] dropping and recreating superpowers_test...');
  const mysqlC = await getContainerName('mysql');
  const redisC = await getContainerName('redis');
  if (!mysqlC) throw new Error('mysql container not found');
  if (!redisC) throw new Error('redis container not found');
  log.gray(`[db] mysql=${mysqlC}  redis=${redisC}`);

  const mysqlArgs = [
    'exec', '-e', 'MYSQL_PWD=rootpass', mysqlC, 'mysql', '-uroot',
    '--show-warnings=0', '-e',
    'DROP DATABASE IF EXISTS superpowers_test; CREATE DATABASE superpowers_test CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;',
  ];
  const mysqlRes = spawnSync('docker', mysqlArgs, { stdio: 'inherit' });
  if (mysqlRes.status !== 0) throw new Error(`mysql reset failed (exit=${mysqlRes.status})`);

  log.cyan('[db] flushing redis...');
  const redisRes = spawnSync('docker', ['exec', redisC, 'redis-cli', 'FLUSHALL'], { stdio: 'inherit' });
  if (redisRes.status !== 0) throw new Error(`redis flush failed (exit=${redisRes.status})`);

  log.green('[db] reset complete (services run on host, no restart needed)');
}

async function waitAllHealthy(timeoutSec = 120) {
  const services = ['mysql', 'redis', 'consul'];
  const start = Date.now();
  const deadline = start + timeoutSec * 1000;
  while (Date.now() < deadline) {
    let allHealthy = true;
    let allRunning = true;
    const buf = await new Promise((resolve) => {
      const p = spawn('docker', composeArgs('ps', '--format', '{{.Service}}|{{.State}}|{{.Health}}'));
      let s = '';
      p.stdout.on('data', (d) => (s += d.toString()));
      p.on('close', () => resolve(s));
    });
    const rows = buf.split(/\r?\n/).filter((l) => l.includes('|'));
    for (const s of services) {
      const row = rows.find((r) => r.startsWith(s + '|'));
      if (!row) { allRunning = false; allHealthy = false; continue; }
      const parts = row.split('|');
      const state = (parts[1] || '').trim();
      const health = (parts[2] || '').trim();
      if (state !== 'running') { allRunning = false; allHealthy = false; continue; }
      if (health && health !== 'healthy') allHealthy = false;
    }
    if (allHealthy && allRunning) {
      log.green('[docker] all services healthy');
      return true;
    }
    const elapsed = Math.round((Date.now() - start) / 1000);
    log.gray(`[docker] waiting... (${elapsed}s elapsed, healthy=${allHealthy} running=${allRunning})`);
    await new Promise((r) => setTimeout(r, 5000));
  }
  log.red(`[docker] TIMEOUT after ${timeoutSec}s`);
  return false;
}

module.exports = {
  COMPOSE_FILE,
  prePull,
  runCompose,
  startTestStack,
  stopTestStack,
  restartServices,
  resetDatabase,
  waitAllHealthy,
};
