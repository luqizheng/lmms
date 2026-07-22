#!/usr/bin/env node
/**
 * scripts/dev-setup.js
 *
 * 一键开发者引导编排：preflight → compose build → compose up → demo-data → ps → summary。
 * 纯 shell-out：check-dev-env / demo-data / compose 都是单独脚本，不重复实现。
 *
 * 用法：
 *   node scripts/dev-setup.js              # 追加模式（默认，幂等 upsert）
 *   node scripts/dev-setup.js --reset      # 转发 --reset 给 demo-data（清业务表再插）
 *
 * 设计注：
 *   - stdio 用 ['inherit', 'pipe', 'pipe'] 而非全 inherit，是为了同时做到
 *     "实时回显" + "失败 ring buffer dump"。pipe 的 chunk 由 tee() 转发到
 *     process.stdout / process.stderr，UX 等同 inherit。
 *   - 摘要框宽 W=95 容下 demo-data.js 的 [demo-data] 前缀剥离后的完整表格行。
 */

const path = require('node:path');
const { spawn } = require('node:child_process');
const { chalk } = require('./lib/ui.js');

const ROOT = path.join(__dirname, '..');
const COMPOSE = ['docker', 'compose', '-f', 'infrastructure/docker-compose.yml'];

const OK = chalk.green('\u2713');
const FAIL = chalk.red('\u2717');

class RingBuffer {
  constructor(n) { this.n = n; this.lines = []; }
  push(line) {
    if (line === '') return;
    this.lines.push(line);
    if (this.lines.length > this.n) this.lines.shift();
  }
  dump() { return this.lines.join('\n'); }
}

function runStep(name, cmd, { capture = false } = {}) {
  return new Promise((resolve, reject) => {
    console.log(`\n${chalk.bold(`[step]`)} ${name}`);
    console.log(`${chalk.dim(`$ ${cmd.join(' ')}`)}`);
    const failBuf = new RingBuffer(50);
    const sumBuf = capture ? new RingBuffer(40) : null;
    const child = spawn(cmd[0], cmd.slice(1), {
      cwd: ROOT, stdio: ['inherit', 'pipe', 'pipe'], env: process.env,
    });
    const tee = (target) => (chunk) => {
      const text = chunk.toString('utf8');
      target.write(text);
      for (const line of text.split('\n')) {
        failBuf.push(line);
        if (sumBuf) sumBuf.push(line);
      }
    };
    child.stdout.on('data', tee(process.stdout));
    child.stderr.on('data', tee(process.stderr));
    child.on('error', reject);
    child.on('close', (code) => {
      if (code !== 0) {
        console.error(`\n${FAIL} ${name} failed (exit ${code})`);
        console.error(`${chalk.dim(`--- last 50 lines ---`)}`);
        console.error(failBuf.dump() || '(no output captured)');
        console.error(`${chalk.dim(`--- end ---`)}`);
        process.exit(code || 1);
      }
      console.log(`${OK} ${name}`);
      resolve(sumBuf ? sumBuf.dump() : '');
    });
  });
}

async function main() {
  const args = process.argv.slice(2);
  const RESET = args.includes('--reset');
  if (args.includes('--help') || args.includes('-h')) {
    console.log('usage: node scripts/dev-setup.js [--reset]\n');
    console.log('  --reset   wipe business tables before seeding (preserves default tenant)');
    process.exit(0);
  }

  console.log(chalk.bold.cyan('=== dev setup ==='));
  console.log(`mode: ${RESET ? 'reset (wipe + seed)' : 'append (idempotent upsert)'}`);

  await runStep('preflight: env check', ['node', 'scripts/check-dev-env.js']);
  await runStep('docker compose build', [...COMPOSE, 'build']);
  await runStep('docker compose up -d', [...COMPOSE, 'up', '-d']);

  const demoArgs = ['node', 'scripts/demo-data.js'];
  if (RESET) demoArgs.push('--reset');
  const demoTail = await runStep('seed demo data', demoArgs, { capture: true });

  await runStep('docker compose ps', [...COMPOSE, 'ps']);

  // ---- summary box ----
  const W = 95;
  const inner = W - 4;
  const row = (s) => `\u2551 ${String(s).slice(0, inner).padEnd(inner)} \u2551`;
  const strip = (s) => s.replace(/^\[demo-data\]\s?/, '');
  const stripped = (demoTail || '(no demo output captured)').split('\n').map(strip);
  console.log(`\n${chalk.green(`\u2554${'='.repeat(W - 2)}\u2557`)}`);
  console.log(`${chalk.green(`${row('Setup complete!')}`)}`);
  console.log(`${chalk.green(`${row('Services  : http://localhost:3000 (api-gateway)')}`)}`);
  console.log(`${chalk.green(`${row('Swagger   : http://localhost:6001..6007/swagger-ui  (per-service)')}`)}`);
  console.log(`${chalk.green(`${row('Login accounts (last ~30 lines from demo-data):')}`)}`);
  for (const ln of stripped) console.log(`${chalk.green(`${row(ln)}`)}`);
  console.log(`${chalk.green(`\u255a${'='.repeat(W - 2)}\u255d`)}`);
}

main().catch((err) => {
  console.error(`${FAIL} dev-setup crashed: ${err.message}`);
  if (err.stack) console.error(err.stack);
  process.exit(1);
});