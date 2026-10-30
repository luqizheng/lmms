'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { resolveTarget, bumpLedger, commitLedger } = require('../lib/release-bump.js');
const { CoderError } = require('../lib/errors.js');

const MK = (v) => `IMAGE_TAG ?= ${v}\n`;
const CL = (v) => `# Changelog\n\n## v${v} (2026-01-01)\n`;

function fixture() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-bump-'));
  for (const [name, content] of [['Makefile', MK('1.0.0')], ['CHANGELOG.md', CL('1.0.0')]]) {
    const p = path.join(dir, 'release', 'dist-template', name);
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, content, 'utf-8');
  }
  return dir;
}

const CONSISTENT = {
  consistent: true,
  drift: [],
  git: { latestTag: 'v1.6.0', imageTag: '1.6.0' },
};
const DRIFTED = {
  consistent: false,
  drift: ['makefile', 'changelog'],
  git: { latestTag: 'v1.6.0', imageTag: '1.6.0' },
};

test('resolveTarget refuses to bump a drifted ledger without --force --from-tag', () => {
  assert.throws(
    () => resolveTarget({ level: 'minor', status: DRIFTED }),
    (err) => {
      assert.ok(err instanceof CoderError);
      assert.equal(err.exitCode, 3);
      assert.equal(err.code, 'VERSION_DRIFT');
      return true;
    },
  );
});

test('resolveTarget normalizes an explicit --version', () => {
  const t = resolveTarget({ status: CONSISTENT, version: 'v1.7.0' });
  assert.equal(t.version, '1.7.0');
  assert.equal(t.imageTag, '1.7.0');
});

test('resolveTarget bumps minor from the git tag baseline', () => {
  const t = resolveTarget({ level: 'minor', status: CONSISTENT });
  assert.equal(t.version, '1.7.0');
  assert.equal(t.imageTag, '1.7.0');
});

test('resolveTarget converges to the tag itself under --force --from-tag', () => {
  const t = resolveTarget({ level: 'minor', status: DRIFTED, force: true, fromTag: true });
  assert.equal(t.imageTag, '1.6.0');
});

test('bumpLedger rewrites exactly the two ledger files', () => {
  const dir = fixture();
  const { changed } = bumpLedger({ root: dir, imageTag: '1.7.0', changelogBody: '### Added\n- coder CLI', date: '2026-10-30' });
  assert.deepEqual(changed, ['release/dist-template/Makefile', 'release/dist-template/CHANGELOG.md']);
  const mk = fs.readFileSync(path.join(dir, 'release', 'dist-template', 'Makefile'), 'utf-8');
  const cl = fs.readFileSync(path.join(dir, 'release', 'dist-template', 'CHANGELOG.md'), 'utf-8');
  assert.match(mk, /IMAGE_TAG \?= 1\.7\.0/);
  assert.match(cl, /^## v1\.7\.0 \(2026-10-30\)/m);
});

test('commitLedger stages the two files and commits without any push', () => {
  const calls = [];
  const exec = (file, args) => { calls.push([file, args]); return ''; };
  const result = commitLedger({ root: '/repo', imageTag: '1.7.0', exec });
  assert.equal(calls.length, 2);
  assert.deepEqual(calls[0][1], ['add', 'release/dist-template/Makefile', 'release/dist-template/CHANGELOG.md']);
  assert.equal(calls[1][1][0], 'commit');
  assert.match(calls[1][1][2], /bump version ledger to 1\.7\.0/);
  const flat = JSON.stringify(calls);
  assert.ok(!flat.includes('push'), 'must never push');
  assert.equal(result.commit, calls[1][1][2]);
});
