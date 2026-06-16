#!/usr/bin/env node
// scripts/run-e2e.js
// 一键启动 docker 全栈并跑 E2E 测试
// 用法: node scripts/run-e2e.js [--filter <name>] [--test-name <pattern>] [--no-up] [--rebuild] [--e2e-debug]

const fs = require('node:fs');
const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');
const { ROOT, log, COLORS } = require('./lib/utils.js');

const REGISTRY_HOST = 'zhcoder-docker-registry.com:8000';
const PROJECT_SERVICES = ['api-gateway', 'token-service'];

function checkImageExists(imageName) {
  const res = spawnSync('docker', ['image', 'inspect', imageName], { encoding: 'utf8' });
  return res.status === 0;
}

function checkImageValid(imageName, entrypoint) {
  // 用 --rm --entrypoint ls 验证镜像内可执行文件是否存在
  const res = spawnSync('docker', ['run', '--rm', '--entrypoint', 'ls', imageName, '-la', entrypoint], {
    encoding: 'utf8',
    timeout: 10000,
  });
  return res.status === 0;
}

function checkAndBuildImages() {
  const missingOrInvalid = [];
  for (const svc of PROJECT_SERVICES) {
    const image = `${REGISTRY_HOST}/micro-server/${svc}:latest`;
    if (!checkImageExists(image)) {
      log.yellow(`  [check] ${image} missing`);
      missingOrInvalid.push(svc);
      continue;
    }
    // 验证可执行文件是否存在
    const entrypoint = `/app/${svc.replace('-service', '')}`;
    if (!checkImageValid(image, entrypoint)) {
      log.yellow(`  [check] ${image} 缺少 ${entrypoint}`);
      missingOrInvalid.push(svc);
    } else {
      log.cyan(`  [check] ${image} OK`);
    }
  }
  if (missingOrInvalid.length > 0) {
    log.yellow(`[check] 发现 ${missingOrInvalid.length} 个镜像缺失/无效，自动构建...`);
    const res = spawnSync('pnpm', ['docker:build', ...missingOrInvalid], {
      cwd: ROOT,
      stdio: 'inherit',
      shell: false,
    });
    if (res.status !== 0) {
      throw new Error(`pnpm docker:build failed with code ${res.status}`);
    }
  }
}

function parseArgs(argv) {
  const out = { noUp: false, filter: null, testName: null, rebuild: false, e2eDebug: false };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--no-up') out.noUp = true;
    else if (a === '--rebuild') out.rebuild = true;
    else if (a === '--e2e-debug') out.e2eDebug = true;
    else if (a === '--filter') out.filter = argv[++i];
    else if (a === '--test-name') out.testName = argv[++i];
  }
  return out;
}

function runCmd(cmd, args, opts = {}) {
  return spawnSync(cmd, args, { stdio: 'inherit', shell: false, ...opts });
}

function printProgressBar(progress, total, label) {
  const barLength = 30;
  const filledLength = Math.round((progress / total) * barLength);
  const bar = '█'.repeat(filledLength) + '░'.repeat(barLength - filledLength);
  const percent = ((progress / total) * 100).toFixed(0);
  process.stdout.write(`\r${COLORS.yellow}  [${bar}] ${percent}%${COLORS.reset} ${label}`);
}

async function waitGatewayHealthy(timeoutSec = 120) {
  const deadline = Date.now() + timeoutSec * 1000;
  const intervalMs = 2000;
  const totalTicks = Math.ceil(timeoutSec / (intervalMs / 1000));
  let tick = 0;
  
  while (Date.now() < deadline) {
    const res = spawnSync('curl', ['-sf', '-o', 'NUL', '-w', '%{http_code}', 'http://localhost:3000/api/v1/auth/setup-status'], { encoding: 'utf8' });
    const code = parseInt(res.stdout || '0', 10);
    if (code >= 200 && code < 500) {
      process.stdout.write('\r' + ' '.repeat(80) + '\r');
      return true;
    }
    
    tick++;
    const progress = Math.min(tick, totalTicks);
    const remainingSec = Math.ceil((deadline - Date.now()) / 1000);
    printProgressBar(progress, totalTicks, `Waiting for gateway... (${remainingSec}s remaining)`);
    
    await new Promise((r) => setTimeout(r, intervalMs));
  }
  
  process.stdout.write('\n');
  return false;
}

async function main() {
  process.chdir(ROOT);
  const opts = parseArgs(process.argv);
  log.cyan('== E2E Docker Runner ==');
  log.cyan(`  NoUp    : ${opts.noUp}`);
  log.cyan(`  Filter  : ${opts.filter}`);
  log.cyan(`  TestName: ${opts.testName}`);
  log.cyan(`  Rebuild : ${opts.rebuild}`);
  log.cyan(`  E2eDebug: ${opts.e2eDebug}`);

  log.yellow('[1/4] Checking e2e dependencies...');
  if (!fs.existsSync(path.join('tests', 'e2e', 'node_modules'))) {
    log.cyan('  Installing pnpm packages...');
    const res = runCmd('pnpm', ['install'], { cwd: path.join('tests', 'e2e') });
    if (res.status !== 0) throw new Error(`pnpm install failed with code ${res.status}`);
  } else {
    log.cyan('  tests/e2e/node_modules exists, skip');
  }

  log.yellow('[2/4] Checking project images...');
  checkAndBuildImages();

  if (!opts.noUp) {
    log.yellow('[3/4] Starting docker compose stack...');
    const composeArgs = ['-f', 'infrastructure/docker-compose.yml', 'up', '-d'];
    if (opts.rebuild) composeArgs.push('--build', '--pull', 'never');
    const up = runCmd('docker', ['compose', ...composeArgs]);
    if (up.status !== 0) throw new Error(`docker compose up failed with code ${up.status}`);

    log.yellow('[3/4] Waiting for API gateway health...');
    if (!(await waitGatewayHealthy(120))) {
      throw new Error('API gateway not healthy after 120s. Run: docker compose -f infrastructure/docker-compose.yml logs');
    }
    log.green('  Gateway is healthy');
  } else {
    log.yellow('[3/4] Skipped (NoUp)');
  }

  log.yellow('[4/4] Running E2E tests...');
  process.env.E2E_BASE_URL = 'http://localhost:3000';
  if (opts.e2eDebug) process.env.E2E_DEBUG = '1';

  const vitestArgs = [];
  if (opts.filter)   vitestArgs.push(opts.filter);
  if (opts.testName) vitestArgs.push('-t', opts.testName);

  const testRes = runCmd('pnpm', ['test', ...vitestArgs], { cwd: path.join('tests', 'e2e') });
  const code = testRes.status ?? 1;

  log.yellow('[4/4] Done.');
  if (code !== 0) log.red(`  vitest exit: ${code}`);
  else log.green('  vitest exit: 0');
  log.plain('');
  log.cyan('To stop the docker stack:');
  log.cyan('  docker compose -f infrastructure/docker-compose.yml down');
  process.exit(code);
}

main().catch((e) => {
  log.red(`error: ${e.message}`);
  process.exit(1);
});
