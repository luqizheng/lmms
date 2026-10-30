'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { readImageTag, applyVersion, bumpMakefileFile } = require('../lib/makefile.js');

const LF = 'IMAGE_TAG ?= 1.0.0\nhelp:\n\t@echo "coder-micro-services v1.0.0 — 可用命令："\n';

test('reads and rewrites LF makefile', () => {
  assert.equal(readImageTag(LF), '1.0.0');
  const out = applyVersion(LF, '1.7.0');
  assert.match(out, /IMAGE_TAG \?= 1\.7\.0/);
  assert.match(out, /coder-micro-services v1\.7\.0/);
  assert.equal(applyVersion(out, '1.7.0'), out, 'idempotent');
});

test('preserves CRLF line endings', () => {
  const CRLF = LF.replaceAll('\n', '\r\n');
  const out = applyVersion(CRLF, '1.7.0');
  assert.ok(out.includes('IMAGE_TAG ?= 1.7.0\r\n'));
  assert.ok(!/[^\r]\n/.test(out), 'no bare LF introduced');
});

test('returns null when IMAGE_TAG line is absent', () => {
  assert.equal(readImageTag('help:\n\t@echo hi\n'), null);
});

test('bumpMakefileFile writes only on change', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-mk-'));
  const file = path.join(dir, 'Makefile');
  fs.writeFileSync(file, LF, 'utf-8');
  assert.equal(bumpMakefileFile(file, '1.7.0'), true);
  assert.equal(bumpMakefileFile(file, '1.7.0'), false, 'second run is a no-op');
  assert.match(fs.readFileSync(file, 'utf-8'), /IMAGE_TAG \?= 1\.7\.0/);
});
