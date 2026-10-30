'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { detectContext } = require('../lib/context.js');

function fixture(files) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-ctx-'));
  for (const [rel, content] of Object.entries(files)) {
    const p = path.join(dir, rel);
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, content);
  }
  return dir;
}

test('detects monorepo by Cargo.toml + services + libs', () => {
  const dir = fixture({ 'Cargo.toml': '[workspace]', 'services/.keep': '', 'libs/.keep': '' });
  assert.equal(detectContext(path.join(dir, 'services')).kind, 'monorepo');
  assert.equal(detectContext(dir).root, dir);
});

test('detects dist by manifest + compose + Makefile', () => {
  const dir = fixture({ 'images/manifest.json': '{}', 'docker-compose.yml': 'x', Makefile: 'IMAGE_TAG ?= 1.0.0' });
  assert.equal(detectContext(dir).kind, 'dist');
});

test('unknown context has null root', () => {
  const dir = fixture({ 'README.md': 'x' });
  assert.deepEqual(detectContext(dir), { kind: 'unknown', root: null });
});
