// Test-Runner.js
// 封装 cargo test，输出统一 JSON
const path = require('node:path');
const fs = require('node:fs');
const { spawn } = require('node:child_process');
const { ROOT } = require('./utils.js');

const SERVICE_MAP = {
  'api-gateway': { lang: 'rust', path: 'services\\api-gateway' },
  'token-service': { lang: 'rust', path: 'services\\token-service' },
  'user-service': { lang: 'rust', path: 'services\\user-service' },
  'notification-service': { lang: 'rust', path: 'services\\notification-service' },
  'audit-service': { lang: 'rust', path: 'services\\audit-service' },
  'config-service': { lang: 'rust', path: 'services\\config-service' },
  'file-service': { lang: 'rust', path: 'services\\file-service' },
};

const ALL_SERVICES = [
  'api-gateway',
  'token-service',
  'user-service',
  'notification-service',
  'audit-service',
  'config-service',
  'file-service',
];

const LIBS = [
  'common-error',
  'common-log',
  'common-config',
  'common-cache',
  'common-db',
  'common-crypto',
  'common-http',
  'common-validator',
  'authz',
  'jwt-ctx',
];

async function runServiceTests(service, type, outputJson) {
  const cfg = SERVICE_MAP[service];
  if (!cfg) throw new Error(`unknown service: ${service}`);
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

async function runLibTests(lib, outputJson) {
  const libRoot = path.join(ROOT, 'libs', lib);
  const prevCwd = process.cwd();
  process.chdir(libRoot);
  try {
    await new Promise((resolve) => {
      const out = fs.openSync(outputJson, 'w');
      const err = fs.openSync(outputJson + '.err', 'w');
      const p = spawn('cargo', ['test'], { stdio: ['ignore', out, err] });
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

function getLibsList() {
  return [...LIBS];
}

module.exports = { SERVICE_MAP, ALL_SERVICES, LIBS, runServiceTests, runLibTests, getServiceList, getLibsList };
