#!/usr/bin/env node
'use strict';

// Build all OpenAPI dump binaries in one Cargo invocation, then execute each
// from its service directory because the binary writes openapi.yaml to cwd.
// Cargo emits compiler-artifact JSON records so we never guess executable paths.

const { spawnSync } = require('node:child_process');
const { existsSync, readFileSync } = require('node:fs');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..');
const services = [
  { dir: 'api-gateway', package: 'api_gateway', bin: 'dump_openapi_api_gateway', title: 'API Gateway' },
  { dir: 'token-service', package: 'token_service', bin: 'dump_openapi_token_service', title: 'token-service' },
  { dir: 'iam-service', package: 'iam_service', bin: 'dump_openapi_iam_service', title: 'iam-service' },
  { dir: 'notification-service', package: 'notification_service', bin: 'dump_openapi_notification_service', title: 'Notification Service' },
  { dir: 'audit-service', package: 'audit_service', bin: 'dump_openapi_audit_service', title: 'audit-service' },
  { dir: 'file-service', package: 'file_service', bin: 'dump_openapi_file_service', title: 'file-service' },
  { dir: 'prue_backend', package: 'prue_backend', bin: 'dump_openapi_prue_backend', title: 'prue_backend' },
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
  if (!pkg || !pkg.targets.some((target) => target.name === service.bin && target.kind.includes('bin'))) {
    console.error(`[openapi] Package or ${service.bin} binary target not found: ${service.package}`);
    process.exit(1);
  }
  packageIds.set(pkg.id, service);
}

console.log(`[openapi] Building ${services.length} uniquely named OpenAPI binaries in one Cargo invocation`);
const buildArgs = [
  'build',
  '--locked',
  '--message-format=json-render-diagnostics',
  ...services.flatMap((service) => ['--package', service.package]),
  ...services.flatMap((service) => ['--bin', service.bin]),
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
  if (message.reason !== 'compiler-artifact' || !message.executable) {
    continue;
  }
  const service = packageIds.get(message.package_id);
  if (service && message.target?.name === service.bin) {
    executables.set(service.package, message.executable);
  }
}

for (const service of services) {
  const executable = executables.get(service.package);
  if (!executable) {
    console.error(`[openapi] Cargo did not report an executable for ${service.package}`);
    process.exit(1);
  }

  console.log(`\n[openapi] Generating services/${service.dir}/openapi.yaml`);
  const serviceDir = path.join(repoRoot, 'services', service.dir);
  const result = spawnSync(executable, [], {
    cwd: serviceDir,
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

  const generated = readFileSync(path.join(serviceDir, 'openapi.yaml'), 'utf8');
  const generatedTitle = generated.match(/^  title:\\s*(.+)$/m)?.[1]?.trim();
  if (generatedTitle !== service.title) {
    console.error(`[openapi] Wrong spec generated for ${service.dir}: expected title "${service.title}", got "${generatedTitle ?? 'missing'}"`);
    process.exit(1);
  }
}

console.log('\n[openapi] All service specs generated successfully.');
