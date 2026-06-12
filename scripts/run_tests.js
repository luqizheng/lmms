#!/usr/bin/env node
// Run Tests Script for coder-micro-services
// Usage: node scripts/run_tests.js [--unit] [--integration] [--all]

const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');
const { ROOT, log } = require('./lib/utils.js');

function parseArgs(argv) {
  const out = { unit: false, integration: false, all: false };
  for (let i = 2; i < argv.length; i++) {
    if (argv[i] === '--unit') out.unit = true;
    else if (argv[i] === '--integration') out.integration = true;
    else if (argv[i] === '--all') out.all = true;
  }
  return out;
}

function writeHeader(service) {
  log.cyan('');
  log.cyan('========================================');
  log.cyan(` Testing: ${service}`);
  log.cyan('========================================');
}

function runCargo(args) {
  return new Promise((resolve) => {
    const p = spawn('cargo', args, { stdio: ['ignore', 'pipe', 'pipe'] });
    let buf = '';
    p.stdout.on('data', (d) => (buf += d.toString()));
    p.stderr.on('data', (d) => (buf += d.toString()));
    p.on('close', () => {
      const lines = buf.split(/\r?\n/).filter((l) => l.length > 0);
      const tail = lines.slice(-10);
      for (const l of tail) process.stdout.write(l + '\n');
      resolve();
    });
  });
}

async function testRustService(servicePath, serviceName) {
  writeHeader(serviceName);
  const prev = process.cwd();
  process.chdir(servicePath);
  try {
    log.yellow('[1/2] Running unit tests...');
    await runCargo(['test', '--lib']);
    log.yellow('[2/2] Running integration tests...');
    await runCargo(['test', '--test', '*']);
  } finally {
    process.chdir(prev);
  }
}

async function main() {
  const opts = parseArgs(process.argv);
  log.green('');
  log.green('===============================================');
  log.green('  coder-micro-services Test Suite');
  log.green('===============================================');

  if (opts.all || (!opts.unit && !opts.integration)) {
    log.magenta('Running ALL tests (unit + integration)...');
    await testRustService(path.join(ROOT, 'services', 'api-gateway'), 'API Gateway (Rust)');
    await testRustService(path.join(ROOT, 'services', 'token-service'), 'Token Service (Rust)');
  } else if (opts.unit) {
    log.magenta('Running UNIT tests only...');
    await testRustService(path.join(ROOT, 'services', 'api-gateway'), 'API Gateway (Rust)');
    await testRustService(path.join(ROOT, 'services', 'token-service'), 'Token Service (Rust)');
  } else if (opts.integration) {
    log.magenta('Running INTEGRATION tests only...');
    log.yellow('Note: Ensure Docker Compose services are running:');
    log.yellow('  cd infrastructure; docker compose -f docker-compose.test-infra.yaml up -d');
    await testRustService(path.join(ROOT, 'services', 'api-gateway'), 'API Gateway (Rust)');
    await testRustService(path.join(ROOT, 'services', 'token-service'), 'Token Service (Rust)');
  }

  log.green('');
  log.green('===============================================');
  log.green('  Test Run Complete');
  log.green('===============================================');
}

main().catch((e) => {
  log.red(`error: ${e.message}`);
  process.exit(1);
});
