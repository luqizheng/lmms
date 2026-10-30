'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { collectStatus } = require('../lib/release-status.js');
const { ExternalError } = require('../lib/errors.js');

function fixture({ makefile, changelog }) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-status-'));
  for (const [name, content] of [['Makefile', makefile], ['CHANGELOG.md', changelog]]) {
    if (content == null) continue;
    const p = path.join(dir, 'release', 'dist-template', name);
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, content, 'utf-8');
  }
  return dir;
}

const MK = (v) => `IMAGE_TAG ?= ${v}\n`;
const CL = (v) => `# Changelog\n\n## v${v} (2026-01-01)\n`;

test('reports drift when ledgers lag behind git tag', () => {
  const dir = fixture({ makefile: MK('1.0.0'), changelog: CL('1.0.0') });
  const status = collectStatus({
    root: dir,
    git: () => 'v1.6.0\narchive/demo-data-fresh-db-register',
    dirty: () => 1,
    branch: () => 'feat/x',
  });
  assert.deepEqual(status.drift.sort(), ['changelog', 'makefile']);
  assert.equal(status.consistent, false);
  assert.equal(status.git.latestTag, 'v1.6.0');
  assert.equal(status.git.imageTag, '1.6.0');
  assert.equal(status.makefile.imageTag, '1.0.0');
});

test('consistent when all three match', () => {
  const dir = fixture({ makefile: MK('1.6.0'), changelog: CL('1.6.0') });
  const status = collectStatus({
    root: dir,
    git: () => 'v1.6.0',
    dirty: () => 0,
    branch: () => 'main',
  });
  assert.equal(status.consistent, true);
  assert.deepEqual(status.drift, []);
});

test('git failure yields all-three drift without throwing', () => {
  const dir = fixture({ makefile: MK('1.0.0'), changelog: CL('1.0.0') });
  const status = collectStatus({
    root: dir,
    git: () => { throw new ExternalError('git failed'); },
    dirty: () => -1,
    branch: () => '(unknown)',
  });
  assert.equal(status.git.latestTag, null);
  assert.deepEqual(status.drift.sort(), ['changelog', 'git', 'makefile']);
  assert.equal(status.consistent, false);
});

test('all-null ledgers are consistent (fresh repo)', () => {
  const dir = fixture({});
  const status = collectStatus({ root: dir, git: () => '', dirty: () => 0, branch: () => 'main' });
  assert.equal(status.consistent, true);
  assert.deepEqual(status.drift, []);
  assert.equal(status.makefile.imageTag, null);
  assert.equal(status.changelog.version, null);
});
