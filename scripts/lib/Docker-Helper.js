// Docker-Helper.js
// 封装 docker compose 与 healthcheck 等待
const { spawn, spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const { log, ROOT } = require('./utils.js');

const COMPOSE_FILE = path.join(ROOT, 'infrastructure', 'docker-compose.yml');

function composeArgs(...extra) {
  return ['compose', '-f', COMPOSE_FILE, ...extra];
}

const PRE_PULL_IMAGES = [
  'alpine:3.21',
  'rust:1.96-slim',
  'mysql:8.0',
  'redis:7-alpine',
  'consul:1.15',
];

const MANIFEST_PATH = path.join(ROOT, 'infrastructure', 'base-images.json');

function loadImageManifest() {
  if (!fs.existsSync(MANIFEST_PATH)) {
    log.yellow(`[pull] manifest not found: ${MANIFEST_PATH}, run: node scripts/base-images-sync.js`);
    return [];
  }
  try {
    const data = JSON.parse(fs.readFileSync(MANIFEST_PATH, 'utf8'));
    return Array.isArray(data.images) ? data.images : [];
  } catch (e) {
    log.yellow(`[pull] manifest parse failed: ${e.message}`);
    return [];
  }
}

async function pullOne(ref, mirror) {
  if (mirror) {
    const mirrorRef = `${mirror.replace(/\/+$/, '')}/${ref}`;
    log.cyan(`[pull] ${mirrorRef} (mirror of ${ref})`);
    const mr = spawnSync('docker', ['pull', mirrorRef], { stdio: 'inherit' });
    if (mr.status === 0) {
      const tr = spawnSync('docker', ['tag', mirrorRef, ref], { stdio: 'inherit' });
      if (tr.status === 0) {
        log.green(`[pull] tagged ${mirrorRef} -> ${ref}`);
        return true;
      }
      log.yellow(`[pull] tag failed, image usable as ${mirrorRef}`);
      return true;
    }
    log.yellow(`[pull] mirror failed, fallback to ${ref}`);
  }
  log.cyan(`[pull] ${ref}`);
  const res = spawnSync('docker', ['pull', ref], { stdio: 'inherit' });
  if (res.status !== 0) {
    log.yellow(`[pull] WARNING: failed to pull ${ref} (will retry during build)`);
    return false;
  }
  return true;
}

async function prePull() {
  const images = loadImageManifest();
  if (images.length === 0) return;
  for (const img of images) {
    try { await pullOne(img.ref, img.mirror); }
    catch (e) { log.yellow(`[pull] ${img.ref} error: ${e.message}`); }
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
  log.cyan('[db] dropping and recreating superpowers...');
  const mysqlC = await getContainerName('mysql');
  const redisC = await getContainerName('redis');
  if (!mysqlC) throw new Error('mysql container not found');
  if (!redisC) throw new Error('redis container not found');
  log.gray(`[db] mysql=${mysqlC}  redis=${redisC}`);

  const mysqlArgs = [
    'exec', '-e', 'MYSQL_PWD=rootpass', mysqlC, 'mysql', '-uroot',
    '--show-warnings=0', '-e',
    'DROP DATABASE IF EXISTS superpowers; CREATE DATABASE superpowers CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;',
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
