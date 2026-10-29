'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');

const { parseArgs, EXIT_USAGE, EXIT_OK } = require('./cli');

test('parseArgs: publish with version, flags and service positionals', () => {
  const p = parseArgs(['publish', '-v', '1.5.2', '--load-and-push', 'api-gateway', 'token-service']);
  assert.equal(p.command, 'publish');
  assert.equal(p.version, '1.5.2');
  assert.equal(p.loadAndPush, true);
  assert.equal(p.push, false);
  assert.deepEqual(p.positionals, ['api-gateway', 'token-service']);
});

test('parseArgs: bump shorthand levels and dry-run', () => {
  const p = parseArgs(['bump', '--minor', '--tag', '--dry-run']);
  assert.equal(p.command, 'bump');
  assert.equal(p.minor, true);
  assert.equal(p.tag, true);
  assert.equal(p.dryRun, true);
  assert.deepEqual(p.positionals, []);
});

test('parseArgs: upgrade positionals', () => {
  const p = parseArgs(['upgrade', '/srv/dist', 'v1.5.3']);
  assert.equal(p.command, 'upgrade');
  assert.deepEqual(p.positionals, ['/srv/dist', 'v1.5.3']);
});

test('parseArgs: value flags consume their argument', () => {
  const p = parseArgs(['publish', '--platform', 'linux/amd64,linux/arm64', '--root', '/repo']);
  assert.equal(p.platform, 'linux/amd64,linux/arm64');
  assert.equal(p.root, '/repo');
  assert.deepEqual(p.positionals, []);
});

test('parseArgs: unknown flag raises usage error', () => {
  assert.throws(() => parseArgs(['publish', '--bogus']), (err) => err.exitCode === EXIT_USAGE);
});

test('parseArgs: missing value for -v raises usage error', () => {
  assert.throws(() => parseArgs(['publish', '-v']), (err) => err.exitCode === EXIT_USAGE);
});

test('parseArgs: help short-circuits without command', () => {
  const p = parseArgs(['--help']);
  assert.equal(p.help, true);
  assert.equal(p.command, null);
  assert.equal(EXIT_OK, 0);
});
