// Test-Runner.js
// 封装 cargo test，输出统一 JSON
const path = require('node:path');
const fs = require('node:fs');
const { spawn } = require('node:child_process');
const { ROOT, log } = require('./utils.js');

const SERVICE_MAP = {
  'api-gateway': { lang: 'rust', path: 'services\\api-gateway' },
  'token-service': { lang: 'rust', path: 'services\\token-service' },
  'credential-login-service': { lang: 'rust', path: 'services\\credential-login-service' },
  'user-service': { lang: 'rust', path: 'services\\user-service' },
  'notification-service': { lang: 'rust', path: 'services\\notification-service' },
};

const ALL_SERVICES = [
  'api-gateway',
  'token-service',
  'credential-login-service',
  'user-service',
  'notification-service',
];

// 跳过 unit 测试的服务（stub/无业务代码）
const SKIP_UNIT_SERVICES = new Set(['credential-login-service']);

async function runServiceTests(service, type, outputJson) {
  const cfg = SERVICE_MAP[service];
  if (!cfg) throw new Error(`unknown service: ${service}`);
  if (type === 'unit' && SKIP_UNIT_SERVICES.has(service)) {
    log.yellow(`[test] ${service} (unit) skipped (no business code)`);
    fs.writeFileSync(outputJson, '');
    return;
  }
  const svcRoot = path.join(ROOT, cfg.path);
  const prevCwd = process.cwd();
  process.chdir(svcRoot);
  // Don't use --message-format=json: it strips libtest's human-readable
  // "test name ... ok/failed/ignored" output that Report-Builder parses.
  const args = type === 'unit' ? ['test', '--lib'] : ['test', '--test', '*'];
  try {
    await new Promise((resolve) => {
      const out = fs.openSync(outputJson, 'w');
      const err = fs.openSync(outputJson + '.err', 'w');
      const p = spawn('cargo', args, { stdio: ['ignore', out, err] });
      p.on('close', () => {
        try { fs.closeSync(out); } catch {}
        try { fs.closeSync(err); } catch {}
        try { fs.unlinkSync(outputJson + '.err'); } catch {}
        resolve();
      });
    });
  } finally {
    process.chdir(prevCwd);
  }
}

function getServiceList() {
  return [...ALL_SERVICES];
}

module.exports = { SERVICE_MAP, ALL_SERVICES, SKIP_UNIT_SERVICES, runServiceTests, getServiceList };
