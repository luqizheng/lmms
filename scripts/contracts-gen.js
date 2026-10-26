#!/usr/bin/env node
/**
 * 契约代码生成入口
 * - 默认：调用 contracts/codegen/*.js 生成各语言代码
 * - --check：生成后用 git diff 检查产物是否漂移，漂移则 exit 1
 */

const path = require('path');
const { execSync } = require('child_process');

const codegenDir = path.resolve(__dirname, '../contracts/codegen');
const repoRoot = path.resolve(__dirname, '..');
const isCheck = process.argv.includes('--check');

console.log('=== Contracts Code Generation ===');
console.log('');

console.log('1. Generating Rust error codes...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-rust.js')}`, { stdio: 'inherit' });
} catch (e) {
  console.error('Failed to generate Rust error codes');
  process.exit(1);
}

console.log('');
console.log('2. Generating Rust permission codes...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-rust-perms.js')}`, { stdio: 'inherit' });
} catch (e) {
  console.error('Failed to generate Rust permission codes');
  process.exit(1);
}

console.log('');
console.log('3. Generating TypeScript contracts...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-ts.js')}`, { stdio: 'inherit' });
} catch (e) {
  console.log('TypeScript generation skipped (frontend not ready)');
}

console.log('');
console.log('4. Generating Rust event structs...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-rust-events.js')}`, { stdio: 'inherit' });
} catch (e) {
  console.error('Failed to generate Rust events');
  process.exit(1);
}

console.log('');
console.log('=== Done ===');

if (isCheck) {
  console.log('');
  console.log('5. Checking for drift...');
  try {
    execSync('git diff --exit-code -- libs/common-error libs/authz libs/common-event packages/contracts/src', {
      cwd: repoRoot,
      stdio: 'inherit',
    });
    console.log('✅ Contracts in sync');
  } catch (e) {
    console.error('');
    console.error('❌ Contract drift detected. Run `pnpm run contracts:gen` and commit the result.');
    process.exit(1);
  }
}
