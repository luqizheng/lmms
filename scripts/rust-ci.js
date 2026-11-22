#!/usr/bin/env node
'use strict';

// Run the Rust CI phases in one process so the shared Cargo target directory is
// reused, while recording wall-clock time for each phase. This is diagnostic:
// it does not skip checks or change test coverage.

const { spawnSync } = require('node:child_process');
const { writeFileSync } = require('node:fs');
const { join } = require('node:path');

const ROOT = join(__dirname, '..');
const REPORT = join(ROOT, 'rust-ci-timings.json');
const skipSlow = process.argv.includes('--skip-slow');
const phases = [
  { name: 'fmt', command: 'cargo', args: ['fmt', '--check', '--all'], slow: false },
  { name: 'clippy', command: 'cargo', args: ['clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'], slow: true },
  { name: 'openapi_generate', command: 'node', args: ['scripts/dump-openapi-specs.js'], slow: true },
  { name: 'openapi_drift', command: 'git', args: ['diff', '--exit-code', '--', 'services/*/openapi.yaml'], slow: true },
  { name: 'unit_tests', command: 'cargo', args: ['test', '--workspace', '--lib'], slow: true },
];

const results = [];
let failed = false;

function run(phase) {
  const startedAt = new Date();
  const start = process.hrtime.bigint();
  const result = spawnSync(phase.command, phase.args, {
    cwd: ROOT,
    stdio: 'inherit',
    env: process.env,
    shell: process.platform === 'win32' && ['node', 'cargo', 'git'].includes(phase.command),
  });
  const durationMs = Number(process.hrtime.bigint() - start) / 1e6;
  const status = result.error ? 'error' : result.status === 0 ? 'passed' : 'failed';
  results.push({
    name: phase.name,
    status,
    duration_ms: Math.round(durationMs),
    started_at: startedAt.toISOString(),
    exit_code: result.status,
    signal: result.signal || null,
    error: result.error ? result.error.message : null,
  });
  console.log(`\n[Rust CI] ${phase.name}: ${status} (${(durationMs / 1000).toFixed(1)}s)`);
  if (status !== 'passed') failed = true;
}

for (const phase of phases) {
  if (skipSlow && phase.slow) {
    results.push({ name: phase.name, status: 'skipped', duration_ms: 0 });
    console.log(`\n[Rust CI] ${phase.name}: skipped (--skip-slow)`);
    continue;
  }
  run(phase);
  if (failed) break;
}

const report = {
  schema_version: 1,
  generated_at: new Date().toISOString(),
  commit: (() => {
    const r = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: ROOT, encoding: 'utf8' });
    return r.status === 0 ? r.stdout.trim() : null;
  })(),
  platform: process.platform,
  architecture: process.arch,
  node_version: process.version,
  rust_ci_status: failed ? 'failed' : 'passed',
  phases: results,
  total_duration_ms: results.reduce((sum, item) => sum + (item.duration_ms || 0), 0),
};

writeFileSync(REPORT, JSON.stringify(report, null, 2) + '\n');
console.log('\n=== Rust CI timing summary ===');
for (const item of results) {
  console.log(`${item.name.padEnd(20)} ${item.status.padEnd(8)} ${(item.duration_ms / 1000).toFixed(1)}s`);
}
console.log(`Report: ${REPORT}`);

if (failed) process.exit(1);
