#!/usr/bin/env node
// 直接在 tests/e2e 目录下用 npx 跑 vitest，stdio 实时透传到当前终端，
// 避免 PowerShell 把 pnpm 子进程的 stderr 当作 NativeCommandError 吞掉输出。
'use strict';
const { spawn } = require('node:child_process');
const { join } = require('node:path');

const e2eDir = join(__dirname, '..', 'tests', 'e2e');
const isWindows = process.platform === 'win32';
const cmd = isWindows ? 'npx.cmd' : 'npx';
const args = ['vitest', 'run', '--reporter=default', ...process.argv.slice(2)];

const child = spawn(cmd, args, {
  cwd: e2eDir,
  stdio: 'inherit',
  env: process.env,
  shell: isWindows,
});

child.on('exit', (code, signal) => {
  if (signal) {
    console.error(`vitest 被信号 ${signal} 终止`);
    process.exit(1);
  }
  process.exit(code ?? 0);
});

child.on('error', (err) => {
  console.error('启动 vitest 失败：', err);
  process.exit(1);
});

