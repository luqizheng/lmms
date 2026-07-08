#!/usr/bin/env node
// e2e-migration-state.js — Phase 10 G3 schema state endpoint smoke.
//
// Tests all 7 service-jwt-gated `GET /<svc>/internal/migrations/state` endpoints:
//   1. Mint a service-jwt with `aud=<svc>` via `POST /tokens/service-jwt`
//      (caller sends that service's pre-shared bootstrap token).
//   2. GET the migration-state endpoint with the minted JWT
//      → expect 200 + valid JSON schema.
//   3. GET without an `Authorization` header
//      → expect 401.
//   4. GET with a service-jwt whose `aud` is a *different* service
//      → expect 401 (Phase 5-B verify_aud_principal_middleware).
//
// Prereqs:
//   - infrastructure/docker-compose-infra.yml infra is up
//     (mysql + rabbitmq + consul + redis)
//   - infrastructure/docker-compose-services.yml services are up
//     (api-gateway + 7 业务服务 + token-service + iam-outbox-publisher)
//   - token-service has the 7 service bootstrap tokens in
//     `SERVICE_BOOTSTRAP_TOKENS` (comma-separated).
//   - This script has network access to all 7 service ports
//     (defaults to localhost; override via `SERVICE_HOST`).
//
// Env:
//   TOKEN_SERVICE_URL   default http://localhost:8082
//   SERVICE_HOST        default localhost
//   TID                 default 1
//   BOOTSTRAP_TOKENS    optional JSON map: {"iam":"t1","ntf":"t2",...}
//                       (per-service env `IAM_BOOTSTRAP_TOKEN` etc. take precedence)
//
// Usage:
//   IAM_BOOTSTRAP_TOKEN=t1 NTF_BOOTSTRAP_TOKEN=t2 ... \
//     node scripts/e2e-migration-state.js
//   (exits 0 on all-pass, 1 on any-fail, 0 with warning if all skipped)

const http = require('node:http');

const TOKEN_SERVICE_URL = process.env.TOKEN_SERVICE_URL || 'http://localhost:8082';
const SERVICE_HOST = process.env.SERVICE_HOST || 'localhost';
const TID = parseInt(process.env.TID || '1', 10);
const TIMEOUT_MS = 10_000;

const SERVICES = [
  { name: 'iam',    path: '/iam/internal/migrations/state',   port: 8081, migrations: 5, dialect: 'mysql' },
  { name: 'ntf',    path: '/ntf/internal/migrations/state',   port: 8083, migrations: 3, dialect: 'mysql' },
  { name: 'token',  path: '/token/internal/migrations/state', port: 8082, migrations: 1, dialect: 'mysql' },
  { name: 'audit',  path: '/audit/internal/migrations/state', port: 8084, migrations: 1, dialect: 'mysql' },
  { name: 'config', path: '/cfg/internal/migrations/state',   port: 8085, migrations: 2, dialect: 'mysql' },
  { name: 'file',   path: '/file/internal/migrations/state',  port: 8086, migrations: 2, dialect: 'mysql' },
  { name: 'prue',   path: '/prue/internal/migrations/state',  port: 8087, migrations: 1, dialect: 'mysql+sqlite' },
];

// Parse BOOTSTRAP_TOKENS JSON env (optional fallback; per-service env wins).
let _bootstrapJsonMap = null;
function parseBootstrapMap() {
  if (_bootstrapJsonMap !== null) return _bootstrapJsonMap;
  if (!process.env.BOOTSTRAP_TOKENS) { _bootstrapJsonMap = {}; return _bootstrapJsonMap; }
  try { _bootstrapJsonMap = JSON.parse(process.env.BOOTSTRAP_TOKENS); }
  catch { _bootstrapJsonMap = {}; }
  return _bootstrapJsonMap;
}

function getBootstrapToken(svcName) {
  const upper = svcName.toUpperCase();
  if (process.env[`${upper}_BOOTSTRAP_TOKEN`]) return process.env[`${upper}_BOOTSTRAP_TOKEN`];
  const map = parseBootstrapMap();
  return map[svcName] || null;
}

function httpRequest(method, url, headers, body) {
  return new Promise((resolve, reject) => {
    const u = new URL(url);
    const opts = {
      method,
      host: u.hostname,
      port: u.port || (u.protocol === 'https:' ? 443 : 80),
      path: u.pathname + (u.search || ''),
      headers: { 'Content-Type': 'application/json', ...(headers || {}) },
      timeout: TIMEOUT_MS,
    };
    const req = http.request(opts, (res) => {
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => {
        const text = Buffer.concat(chunks).toString('utf8');
        let json = null;
        try { json = text ? JSON.parse(text) : null; } catch { /* leave null */ }
        resolve({ status: res.statusCode, body: json, text });
      });
    });
    req.on('timeout', () => req.destroy(new Error(`timeout ${TIMEOUT_MS}ms`)));
    req.on('error', reject);
    if (body) req.write(typeof body === 'string' ? body : JSON.stringify(body));
    req.end();
  });
}

function ok(label) { return `\x1b[32m✓\x1b[0m ${label}`; }
function fail(label, detail) { return `\x1b[31m✗\x1b[0m ${label}\n   ${detail}`; }
function warn(label, detail) { return `\x1b[33m⚠\x1b[0m ${label}\n   ${detail}`; }

async function mintServiceJwt(aud, bootstrapToken) {
  const res = await httpRequest(
    'POST',
    `${TOKEN_SERVICE_URL}/tokens/service-jwt`,
    { Authorization: `Bearer ${bootstrapToken}` },
    { aud, tid: TID },
  );
  if (res.status !== 200 || !res.body?.access_token) {
    throw new Error(`mint aud=${aud} failed: ${res.status} ${res.text}`);
  }
  return res.body.access_token;
}

async function callMigrationState(svc, jwt) {
  const url = `http://${SERVICE_HOST}:${svc.port}${svc.path}`;
  return httpRequest('GET', url, jwt ? { Authorization: `Bearer ${jwt}` } : {}, null);
}

function validateResponse(svc, body) {
  const errors = [];
  if (!body) return ['empty body'];
  if (body.service !== svc.name) errors.push(`service field expected "${svc.name}", got "${body.service}"`);
  if (body.dialect !== svc.dialect) errors.push(`dialect field expected "${svc.dialect}", got "${body.dialect}"`);
  if (!Array.isArray(body.available) || body.available.length !== svc.migrations) {
    errors.push(`available expected ${svc.migrations} entries, got ${body.available?.length}`);
  }
  if (!Array.isArray(body.applied)) errors.push(`applied not an array`);
  if (typeof body.in_sync !== 'boolean') errors.push(`in_sync not boolean`);
  if (body.current_version !== null && typeof body.current_version !== 'string') {
    errors.push(`current_version must be string or null`);
  }
  for (const m of body.available || []) {
    if (typeof m.version !== 'string' || typeof m.name !== 'string') {
      errors.push(`available entry missing version/name: ${JSON.stringify(m)}`);
      break;
    }
  }
  return errors;
}

async function checkService(svc) {
  const lines = [];
  const bootstrap = getBootstrapToken(svc.name);
  if (!bootstrap) {
    lines.push(warn(`${svc.name}: no bootstrap token`, `set ${svc.name.toUpperCase()}_BOOTSTRAP_TOKEN or BOOTSTRAP_TOKENS env`));
    return { svc: svc.name, skipped: true, lines };
  }

  // 1. Mint service-jwt
  let jwt;
  try {
    jwt = await mintServiceJwt(svc.name, bootstrap);
    lines.push(ok(`${svc.name}: mint service-jwt aud=${svc.name}`));
  } catch (e) {
    lines.push(fail(`${svc.name}: mint service-jwt failed`, e.message));
    return { svc: svc.name, skipped: false, lines };
  }

  // 2. Call with correct jwt
  try {
    const res = await callMigrationState(svc, jwt);
    if (res.status !== 200) {
      lines.push(fail(`${svc.name}: GET with correct jwt → HTTP ${res.status}`, res.text?.slice(0, 200) || ''));
    } else {
      const errors = validateResponse(svc, res.body);
      if (errors.length === 0) {
        lines.push(ok(`${svc.name}: GET 200 schema=ok (current_version=${res.body.current_version}, in_sync=${res.body.in_sync}, applied=${res.body.applied.length}/${res.body.available.length})`));
      } else {
        lines.push(fail(`${svc.name}: schema validation`, errors.join('; ')));
      }
    }
  } catch (e) {
    lines.push(fail(`${svc.name}: GET failed`, e.message));
  }

  // 3. No token → 401
  try {
    const res = await callMigrationState(svc, null);
    if (res.status === 401) {
      lines.push(ok(`${svc.name}: no token → 401`));
    } else {
      lines.push(fail(`${svc.name}: no token expected 401, got ${res.status}`, res.text?.slice(0, 200) || ''));
    }
  } catch (e) {
    lines.push(fail(`${svc.name}: no-token test failed`, e.message));
  }

  // 4. Wrong-aud jwt → 401
  const otherSvc = SERVICES.find((s) => s.name !== svc.name && getBootstrapToken(s.name));
  if (otherSvc) {
    try {
      const wrongAudJwt = await mintServiceJwt(otherSvc.name, getBootstrapToken(otherSvc.name));
      const res = await callMigrationState(svc, wrongAudJwt);
      if (res.status === 401) {
        lines.push(ok(`${svc.name}: wrong-aud jwt (aud=${otherSvc.name}) → 401`));
      } else {
        lines.push(fail(`${svc.name}: wrong-aud expected 401, got ${res.status}`, res.text?.slice(0, 200) || ''));
      }
    } catch (e) {
      lines.push(fail(`${svc.name}: wrong-aud test failed`, e.message));
    }
  }

  return { svc: svc.name, skipped: false, lines };
}

async function main() {
  console.log('[e2e-migration-state] Phase 10 G3 schema state endpoint smoke');
  console.log(`  token-service: ${TOKEN_SERVICE_URL}`);
  console.log(`  service host:  ${SERVICE_HOST}`);
  console.log(`  tid:           ${TID}`);
  console.log(`  services:      ${SERVICES.length}`);
  console.log('');

  const results = await Promise.all(SERVICES.map(checkService));

  for (const r of results) {
    console.log(`\n[${r.svc}]${r.skipped ? ' (skipped — no bootstrap token)' : ''}`);
    for (const line of r.lines) console.log('  ' + line);
  }

  const all = results.flatMap((r) => r.lines);
  const passed = all.filter((l) => l.startsWith('\x1b[32m')).length;
  const failed = all.filter((l) => l.startsWith('\x1b[31m')).length;
  const skipped = all.filter((l) => l.startsWith('\x1b[33m')).length;
  const skippedSvcs = results.filter((r) => r.skipped).length;

  console.log('');
  if (failed === 0 && skippedSvcs === 0) {
    console.log(`\x1b[32m✓ all ${passed} checks pass across ${SERVICES.length} services\x1b[0m`);
    process.exit(0);
  } else if (failed === 0) {
    console.log(`\x1b[33m⚠ ${skippedSvcs} services skipped (no bootstrap token); ${passed} checks pass; ${skipped} warnings\x1b[0m`);
    process.exit(0);
  } else {
    console.log(`\x1b[31m✗ ${failed} checks fail across ${SERVICES.length - skippedSvcs} services\x1b[0m`);
    process.exit(1);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});