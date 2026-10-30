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

test('--version prints package version and exits 0', async () => {
  const io = capture();
  const code = await run(['--version'], { ...io });
  assert.equal(code, EXIT.OK);
  assert.match(io.stdout.text.trim(), /^\d+\.\d+\.\d+/);
});

test('unknown command exits USAGE with json envelope on stdout only', async () => {
  const io = capture();
  const code = await run(['bogus', '--json'], { ...io });
  assert.equal(code, EXIT.USAGE);
  const lines = io.stdout.text.trim().split('\n');
  assert.equal(lines.length, 1);
  const env = JSON.parse(lines[0]);
  assert.equal(env.command, 'cli');
  assert.equal(env.ok, false);
  assert.equal(env.exitCode, EXIT.USAGE);
  assert.equal(env.errors[0].code, 'USAGE');
  assert.equal(io.stderr.text, '');
});

test('json mode keeps human logs off stdout', async () => {
  const io = capture();
  await run(['version', '--json'], { ...io });
  const env = JSON.parse(io.stdout.text.trim());
  assert.equal(env.command, 'version');
  assert.equal(env.ok, true);
  assert.equal(typeof env.data.node, 'string');
});
