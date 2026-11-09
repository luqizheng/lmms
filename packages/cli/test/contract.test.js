'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { run } = require('../lib/cli.js');
const { EXIT } = require('../lib/errors.js');

function capture() {
  const out = { text: '', write(s) { this.text += s; return true; } };
  const err = { text: '', write(s) { this.text += s; return true; } };
  return { stdout: out, stderr: err };
}

// --- Regression: `coder assets download --help` used to run the download
// instead of printing help. An agent probing usage must not trigger a
// real side effect.

test('--help after the command name prints help without running the command', async () => {
  const io = capture();
  const code = await run(['assets', 'download', '--help'], { ...io });
  assert.equal(code, EXIT.OK);
  assert.match(io.stdout.text, /usage: coder assets download/);
  assert.doesNotMatch(io.stdout.text, /downloaded|already present/,
    'must not have executed the download');
});

test('--help before the command name still works', async () => {
  const io = capture();
  const code = await run(['--help'], { ...io });
  assert.equal(code, EXIT.OK);
  assert.match(io.stdout.text, /usage: coder/);
});

test('--help after the command name works for a command with no side effects', async () => {
  const io = capture();
  const code = await run(['version', '--help'], { ...io });
  assert.equal(code, EXIT.OK);
  assert.match(io.stdout.text, /usage: coder version/);
});

// --- Regression: `coder assets download --totally-bogus` must exit USAGE
// A typo must be loud.

test('unknown flag on a spec-backed command exits USAGE', async () => {
  const io = capture();
  const code = await run(['assets', 'download', '--totally-bogus'], { ...io });
  assert.equal(code, EXIT.USAGE);
  assert.match(io.stderr.text, /--totally-bogus/);
});

test('unknown flag in json mode reports UNKNOWN_FLAG on stdout envelope', async () => {
  const io = capture();
  const code = await run(['assets', 'download', '--totally-bogus', '--json'], { ...io });
  assert.equal(code, EXIT.USAGE);
  const lines = io.stdout.text.trim().split('\n');
  assert.equal(lines.length, 1, 'stdout must stay a single envelope line');
  const env = JSON.parse(lines[0]);
  assert.equal(env.ok, false);
  assert.equal(env.exitCode, EXIT.USAGE);
  assert.equal(env.errors[0].code, 'UNKNOWN_FLAG');
});

test('unknown command reports UNKNOWN_COMMAND', async () => {
  const io = capture();
  const code = await run(['bogus', '--json'], { ...io });
  assert.equal(code, EXIT.USAGE);
  const env = JSON.parse(io.stdout.text.trim());
  assert.equal(env.errors[0].code, 'UNKNOWN_COMMAND');
});

test('a known flag still works after the spec layer is in place', async () => {
  const io = capture();
  const code = await run(['assets', 'download', '--json'], { ...io });
  // assets download without network may fail, but spec parse must succeed (exitCode != USAGE)
  assert.ok(code !== EXIT.USAGE, `expected non-USAGE exit, got ${code}`);
  const env = JSON.parse(io.stdout.text.trim());
  assert.equal(env.command, 'assets download');
});

test('commands without a declared spec keep working (backward compatible)', async () => {
  const io = capture();
  const code = await run(['version', '--json'], { ...io });
  assert.equal(code, EXIT.OK);
  const env = JSON.parse(io.stdout.text.trim());
  assert.equal(env.command, 'version');
});
