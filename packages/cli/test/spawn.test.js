'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { makeExec } = require('../lib/spawn.js');

const PRINT = [process.execPath, '-e', 'console.log("child-stdout-line"); process.stderr.write("child-stderr-line")'];

function captureStdout(fn) {
  const orig = process.stdout.write.bind(process.stdout);
  let leaked = '';
  process.stdout.write = (chunk) => { leaked += String(chunk); return true; };
  try {
    return { result: fn(), leaked };
  } finally {
    process.stdout.write = orig;
  }
}

test('json mode: child output is diverted away from process.stdout', () => {
  const exec = makeExec({ json: true });
  const { result, leaked } = captureStdout(() => exec(PRINT[0], PRINT.slice(1), {}));
  assert.equal(leaked, '', 'nothing may leak to stdout in json mode');
  assert.match(String(result.stdout), /child-stdout-line/);
  assert.ok((result.status ?? 0) === 0);
});

test('shell passthrough resolves .cmd shims (pnpm) on win32', () => {
  const exec = makeExec({ json: false });
  const { result } = captureStdout(() => exec('pnpm', ['-v'], { shell: process.platform === 'win32', stdio: 'pipe', encoding: 'utf-8' }));
  assert.ok((result.status ?? 1) === 0, `pnpm -v failed: ${result.error?.message ?? result.stderr}`);
});

test('human mode (default): child inherits stdio, nothing rerouted through process.stdout', () => {
  const exec = makeExec({});
  const { result, leaked } = captureStdout(() => exec(process.execPath, ['-e', 'process.stdout.write("visible")']));
  assert.equal(leaked, '', 'inherit mode writes straight to the fd, not via process.stdout.write');
  assert.ok((result.status ?? 0) === 0);
});
