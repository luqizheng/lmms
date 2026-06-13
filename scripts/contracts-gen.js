#!/usr/bin/env node
/**
 * 契约代码生成入口
 * 调用 contracts/codegen/*.js 生成各语言代码
 */

const path = require('path');
const { execSync } = require('child_process');

const codegenDir = path.resolve(__dirname, '../contracts/codegen');

console.log('=== Contracts Code Generation ===');
console.log('');

// 生成 Rust 错误码
console.log('1. Generating Rust error codes...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-rust.js')}`, { stdio: 'inherit' });
} catch (e) {
  console.error('Failed to generate Rust error codes');
  process.exit(1);
}

// 生成 Rust 权限码
console.log('');
console.log('2. Generating Rust permission codes...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-rust-perms.js')}`, { stdio: 'inherit' });
} catch (e) {
  console.error('Failed to generate Rust permission codes');
  process.exit(1);
}

// 生成 TypeScript（预留）
console.log('');
console.log('3. Generating TypeScript contracts...');
try {
  execSync(`node ${path.join(codegenDir, 'gen-ts.js')}`, { stdio: 'inherit' });
} catch (e) {
  // TypeScript 生成失败不阻断（预留状态）
  console.log('TypeScript generation skipped (frontend not ready)');
}

console.log('');
console.log('=== Done ===');