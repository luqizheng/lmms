#!/usr/bin/env node
'use strict';

// Generate OpenAPI specs from each Rust service's dump_openapi binary.
// Run each package from its own directory because the binary writes
// openapi.yaml relative to the current working directory. Cargo still uses
// the workspace-level target directory, so compiled dependencies are reused.

const { spawnSync } = require('node:child_process');
const { existsSync } = require('node:fs');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..');
const services = [
  'api-gateway',
  'token-service',
  'iam-service',
  'notification-service',
  'audit-service',
  'file-service',
  'prue_backend',
];

for (const service of services) {
  const serviceDir = path.join(repoRoot, 'services', service);
  const manifest = path.join(serviceDir, 'Cargo.toml');
  const binary = path.join(serviceDir, 'src', 'bin', 'dump-openapi.rs');

  if (!existsSync(manifest) || !existsSync(binary)) {
    console.error(`[openapi] Missing Cargo manifest or dump_openapi binary source for ${service}`);
    process.exit(1);
  }

  console.log(`\n[openapi] Generating services/${service}/openapi.yaml`);
  const result = spawnSync(
    'cargo',
    ['run', '--locked', '--manifest-path', 'Cargo.toml', '--bin', 'dump_openapi'],
    { cwd: serviceDir, stdio: 'inherit' },
  );

  if (result.error) {
    console.error(`[openapi] Failed to start Cargo for ${service}: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    console.error(`[openapi] Generation failed for ${service} (exit ${result.status ?? result.signal ?? 'unknown'})`);
    process.exit(result.status || 1);
  }
}

console.log('\n[openapi] All service specs generated successfully.');
