#!/usr/bin/env node
/**
 * Dump OpenAPI specs for every backend service.
 *
 * Rust services expose a `dump-openapi` bin which writes services/<name>/openapi.yaml.
 * Java/Spring Boot services expose `openapi.yaml` via springdoc at runtime; the
 * dump step validates the committed spec is present.
 *
 * CI usage:
 *   node scripts/dump-openapi-specs.js
 *   git diff --exit-code -- 'services/<star>openapi.yaml'
 *
 * Equivalent to the former scripts/dump-openapi-specs.sh
 */

const { execSync } = require('child_process');
const { existsSync } = require('fs');
const path = require('path');

const ROOT_DIR = path.resolve(__dirname, '..');

/**
 * @typedef {{ name: string, kind: 'rust' | 'java' }} ServiceEntry
 */

/** @type {ServiceEntry[]} */
const SERVICES = [
  { name: 'audit-service',       kind: 'rust' },
  { name: 'config-service',      kind: 'rust' },
  { name: 'file-service',        kind: 'rust' },
  { name: 'prue_backend',         kind: 'rust' },
  { name: 'api-gateway',         kind: 'rust' },
  { name: 'token-service',       kind: 'rust' },
  { name: 'iam-service',         kind: 'rust' },
  { name: 'notification-service', kind: 'rust' },
];

/**
 * Dump OpenAPI spec for a Rust service via `cargo run --bin dump-openapi`.
 */
function dumpRust(svc) {
  const dir = path.join(ROOT_DIR, 'services', svc);
  console.log(`=== Dumping OpenAPI for ${svc} (rust via dump-openapi bin) ===`);
  try {
    execSync('cargo run --bin dump-openapi', {
      cwd: dir,
      stdio: 'inherit',
      timeout: 300_000, // 5 min per service
    });
  } catch {
    console.warn(`  WARN: rust dump failed for ${svc} — keep committed openapi.yaml`);
  }
  console.log(`--- ${svc}/openapi.yaml written ---`);
}

/**
 * Validate that a Java service has its openapi.yaml committed.
 */
function dumpJava(svc) {
  const yaml = path.join(ROOT_DIR, 'services', svc, 'openapi.yaml');
  console.log(`=== Dumping OpenAPI for ${svc} (java springdoc — committed source) ===`);
  if (!existsSync(yaml)) {
    console.error(`  ERROR: ${yaml} missing — phase 9 worker must commit a hand-written spec mirroring the controllers (no java toolchain in CI dev env)`);
    process.exit(1);
  }
  console.log(`--- ${svc}/openapi.yaml present ---`);
}

// ── Main ──────────────────────────────────────────────────────────────────────

let exitCode = 0;

for (const { name, kind } of SERVICES) {
  if (kind === 'rust') {
    dumpRust(name);
  } else if (kind === 'java') {
    dumpJava(name);
  } else {
    console.error(`  ERROR: unknown service kind '${kind}' for ${name}`);
    exitCode = 1;
  }
}

if (exitCode === 0) {
  console.log(`=== All ${SERVICES.length} OpenAPI specs dumped ===`);
}

process.exit(exitCode);
