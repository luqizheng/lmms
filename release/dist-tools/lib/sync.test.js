'use strict';

/**
 * sync.test.js — verify skip semantics:
 *
 *   1. managed wins over skip (explicit > wildcard)
 *   2. skip excludes merge files (no accidental force-merge)
 *   3. managed-vs-skip overlaps are reported
 *   4. files in neither managed nor merge nor skip are untouched
 */

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');

// Mirror of sync.js#sidecarPath so we can test the naming convention in
// isolation. Keep in sync with the production implementation.
function sidecarPath(dstPath) {
  const ext = path.extname(dstPath);
  const base = dstPath.slice(0, dstPath.length - ext.length);
  return base + '.new' + ext;
}

// We test the policy-application logic by extracting it into a tiny harness
// (re-implementing the filter step from sync.js here so we don't need a full
// tarball source). When sync.js's filter step changes, mirror it here.

function applySkip(rawManaged, rawMerge, skipSet) {
  const overlaps = rawManaged.filter((f) => skipSet.has(f));
  const managedFiles = rawManaged;
  const mergeFiles = rawMerge.filter((f) => !skipSet.has(f));
  return { managedFiles, mergeFiles, overlaps };
}

function expandGlobs(patterns, files) {
  if (!patterns || patterns.length === 0) return [];
  const out = new Set();
  for (const pat of patterns) {
    const re = globToRegex(pat);
    for (const f of files) {
      if (re.test(f)) out.add(f);
    }
  }
  return [...out].sort();
}

function globToRegex(pattern) {
  let out = '^';
  let i = 0;
  while (i < pattern.length) {
    const c = pattern[i];
    if (c === '*' && pattern[i + 1] === '*') {
      out += '.*';
      i += 2;
      if (pattern[i] === '/') i++;
    } else if (c === '*') {
      out += '[^/]*';
      i++;
    } else if (c === '?') {
      out += '[^/]';
      i++;
    } else if ('.+^${}()|[]\\'.includes(c)) {
      out += '\\' + c;
      i++;
    } else {
      out += c;
      i++;
    }
  }
  out += '$';
  return new RegExp(out);
}

test('skip excludes merge files when no managed override', () => {
  const files = [
    'contracts/api.yaml',
    'contracts/secret.yaml',
    'scripts/build.sh',
  ];
  const skipSet = new Set(expandGlobs(['contracts/secret.yaml'], files));
  const managed = expandGlobs(['scripts/**'], files);
  const merge = expandGlobs(['contracts/**'], files);

  const { managedFiles, mergeFiles, overlaps } = applySkip(managed, merge, skipSet);
  assert.deepEqual(managedFiles, ['scripts/build.sh']);
  assert.deepEqual(mergeFiles, ['contracts/api.yaml']); // secret.yaml excluded
  assert.deepEqual(overlaps, []);
});

test('managed wins over skip (explicit > wildcard)', () => {
  const files = [
    'frontend/nginx.conf',
    'frontend/vite.config.ts',
    'frontend/src/main.ts',
  ];
  const skipSet = new Set(expandGlobs(['frontend/**'], files));
  const managed = expandGlobs(['frontend/nginx.conf', 'frontend/vite.config.ts'], files);
  const merge = [];

  const { managedFiles, mergeFiles, overlaps } = applySkip(managed, merge, skipSet);
  assert.deepEqual(
    managedFiles.sort(),
    ['frontend/nginx.conf', 'frontend/vite.config.ts']
  );
  assert.deepEqual(mergeFiles, []);
  // Both managed files match skip → reported as overlaps
  assert.deepEqual(
    overlaps.sort(),
    ['frontend/nginx.conf', 'frontend/vite.config.ts']
  );
});

test('files not in any list are untouched (implicit skip)', () => {
  const files = ['apps/web/main.ts', 'infrastructure/keep-this.yaml'];
  const skipSet = new Set();
  const managed = [];
  const merge = expandGlobs(['contracts/**'], files);

  const { managedFiles, mergeFiles, overlaps } = applySkip(managed, merge, skipSet);
  assert.deepEqual(managedFiles, []);
  assert.deepEqual(mergeFiles, []); // no contract files
  assert.deepEqual(overlaps, []);
  // apps/ and infrastructure/keep-this.yaml are not in any list → untouched
});

test('skip with no managed/merge entries is a no-op', () => {
  const files = ['apps/x.ts', 'infrastructure/y.toml'];
  const skipSet = new Set(expandGlobs(['apps/**'], files));
  const managed = [];
  const merge = [];

  const { managedFiles, mergeFiles, overlaps } = applySkip(managed, merge, skipSet);
  assert.deepEqual(managedFiles, []);
  assert.deepEqual(mergeFiles, []);
  assert.deepEqual(overlaps, []);
});
// sidecarPath naming: <file>.new.<ext>, extensionless stays extensionless
test('sidecarPath inserts .new before extension', () => {
  assert.equal(sidecarPath('/p/docker-compose.yml'), '/p/docker-compose.new.yml');
  assert.equal(sidecarPath('/p/docker-compose-services.yml'), '/p/docker-compose-services.new.yml');
  assert.equal(sidecarPath('/p/Makefile'), '/p/Makefile.new');
  assert.equal(sidecarPath('/p/script.sh'), '/p/script.new.sh');
  assert.equal(sidecarPath('/p/.env.example'), '/p/.env.new.example');
});

// protected-managed: mirror of sync.js's three branches:
//   dst missing       → create (copy as-is)
//   dst exists, same  → unchanged
//   dst exists, diff  → sidecar (write src as <file>.new.<ext>)
function planProtected(rel, srcContent, dstContent) {
  const identical = srcContent === dstContent;
  if (dstContent === null) return { action: 'create', rel };
  if (identical) return { action: 'unchanged', rel };
  return { action: 'sidecar', rel, newRel: rel.replace(/(\.[^./]+)?$/, '.new$1') };
}

test('protected-managed: dst missing → create', () => {
  const plan = planProtected('infrastructure/docker-compose.yml', 'upstream', null);
  assert.equal(plan.action, 'create');
  assert.equal(plan.rel, 'infrastructure/docker-compose.yml');
});

test('protected-managed: dst identical → unchanged', () => {
  const plan = planProtected('infrastructure/docker-compose.yml', 'same', 'same');
  assert.equal(plan.action, 'unchanged');
});

test('protected-managed: dst differs → sidecar (.new.<ext>)', () => {
  const plan = planProtected('infrastructure/docker-compose.yml', 'upstream-new', 'downstream-local');
  assert.equal(plan.action, 'sidecar');
  assert.equal(plan.newRel, 'infrastructure/docker-compose.new.yml');
});

test('protected-managed: extensionless filename → Makefile.new', () => {
  const plan = planProtected('Makefile', 'new', 'old');
  assert.equal(plan.action, 'sidecar');
  assert.equal(plan.newRel, 'Makefile.new');
});
