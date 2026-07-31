'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const { parsePolicy } = require('./parse-policy.js');

test('parses managed/three-way-merge/skip blocks', () => {
  const yaml = `
managed:
  - .cargo/config.toml
  - bin/*.sh

three-way-merge:
  - contracts/**
  - scripts/**

skip:
  - .gitignore
  - apps/**
`;
  const result = parsePolicy(yaml);
  assert.deepEqual(result.managed, ['.cargo/config.toml', 'bin/*.sh']);
  assert.deepEqual(result.merge, ['contracts/**', 'scripts/**']);
  assert.deepEqual(result.skip, ['.gitignore', 'apps/**']);
});

test('strips comments and blank lines', () => {
  const yaml = `
# this is a comment
managed:
  # inline comment
  - .cargo/config.toml

  - pnpm-workspace.yaml
`;
  const result = parsePolicy(yaml);
  assert.deepEqual(result.managed, ['.cargo/config.toml', 'pnpm-workspace.yaml']);
});

test('strips quoted values', () => {
  const yaml = `
managed:
  - ".cargo/config.toml"
  - 'bin/*.sh'
`;
  const result = parsePolicy(yaml);
  assert.deepEqual(result.managed, ['.cargo/config.toml', 'bin/*.sh']);
});

test('returns empty arrays for missing sections', () => {
  const yaml = `
managed:
  - .cargo/config.toml
`;
  const result = parsePolicy(yaml);
  assert.deepEqual(result.managed, ['.cargo/config.toml']);
  assert.deepEqual(result.merge, []);
  assert.deepEqual(result.skip, []);
});

test('ignores malformed lines but continues', () => {
  const yaml = `
managed:
  - .cargo/config.toml
this is not valid yaml structure
  - pnpm-workspace.yaml
`;
  const result = parsePolicy(yaml);
  assert.deepEqual(result.managed, ['.cargo/config.toml', 'pnpm-workspace.yaml']);
});