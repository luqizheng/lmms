#!/usr/bin/env node
'use strict';

// Build all OpenAPI dump binaries in one Cargo invocation, then execute each
// from its service directory because the binary writes openapi.yaml to cwd.
// Cargo emits compiler-artifact JSON records so we never guess executable paths.

const { spawnSync } = require('node:child_process');
const { existsSync } = require('node:fs');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..');
const services = [
  { dir: 'api-gateway', package: 'api_gateway' },
  { dir: 'token-service', package: 'token_service' },
  { dir: 'iam-service', package: 'iam_service' },
  { dir: 'notification-service', package: 'notification_service' },
  { dir: 'audit-service', package: 'audit_service' },
  { dir: 'file-service', package: 'file_service' },
  { dir: 'prue_backend', package: 'prue_backend' },
];

function runCargo(args, options = {}) {
  const result = spawnSync('cargo', args, {
    cwd: repoRoot,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    ...options,
  });

  if (result.error) {
    console.error(`[openapi] Failed to start Cargo: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    if (result.stdout) process.stdout.write(result.stdout);
    if (result.stderr) process.stderr.write(result.stderr);
    console.error(`[openapi] Cargo failed (exit ${result.status ?? result.signal ?? 'unknown'})`);
    process.exit(result.status || 1);
  }
  return result;
}

for (const service of services) {
  const serviceDir = path.join(repoRoot, 'services', service.dir);
  const manifest = path.join(serviceDir, 'Cargo.toml');
  const binary = path.join(serviceDir, 'src', 'bin', 'dump-openapi.rs');

  if (!existsSync(manifest) || !existsSync(binary)) {
    console.error(`[openapi] Missing Cargo manifest or dump_openapi binary source for ${service.dir}`);
    process.exit(1);
  }
}

// Resolve package IDs from Cargo metadata rather than relying on target/debug
// naming conventions, which can collide when multiple packages use the same bin.
const metadataResult = runCargo(['metadata', '--locked', '--no-deps', '--format-version', '1']);
let metadata;
try {
  metadata = JSON.parse(metadataResult.stdout);
} catch (error) {
  console.error(`[openapi] Could not parse cargo metadata: ${error.message}`);
  process.exit(1);
}

const packageIds = new Map();
for (const service of services) {
  const pkg = metadata.packages.find((item) => item.name === service.package);
  if (!pkg || !pkg.targets.some((target) => target.name === 'dump_openapi' && target.kind.includes('bin'))) {
    console.error(`[openapi] Package or dump_openapi binary target not found: ${service.package}`);
    process.exit(1);
  }
  packageIds.set(pkg.id, service);
}

console.log(`[openapi] Building ${services.length} dump_openapi binaries in one Cargo invocation`);
const buildArgs = [
  'build',
  '--locked',
  '--message-format=json-render-diagnostics',
  ...services.flatMap((service) => ['--package', service.package]),
  '--bin',
  'dump_openapi',
];
const buildResult = runCargo(buildArgs, { stdio: ['ignore', 'pipe', 'inherit'] });

const executables = new Map();
for (const line of buildResult.stdout.split(/\r?\n/)) {
  if (!line) continue;
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    continue;
  }
  if (message.reason !== 'compiler-artifact' || message.target?.name !== 'dump_openapi' || !message.executable) {
    continue;
  }
  const service = packageIds.get(message.package_id);
  if (service) executables.set(service.package, message.executable);
}

for (const service of services) {
  const executable = executables.get(service.package);
  if (!executable) {
    console.error(`[openapi] Cargo did not report an executable for ${service.package}`);
    process.exit(1);
  }

  console.log(`\n[openapi] Generating services/${service.dir}/openapi.yaml`);
  const result = spawnSync(executable, [], {
    cwd: path.join(repoRoot, 'services', service.dir),
    stdio: 'inherit',
  });

  if (result.error) {
    console.error(`[openapi] Failed to start ${service.package}: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    console.error(`[openapi] Generation failed for ${service.dir} (exit ${result.status ?? result.signal ?? 'unknown'})`);
    process.exit(result.status || 1);
  }
}

console.log('\n[openapi] All service specs generated successfully.');
