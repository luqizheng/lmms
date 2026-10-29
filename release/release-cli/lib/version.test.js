'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');

const { normalizeVersion, compareVersions, bumpVersion, latestTag } = require('./version');

test('normalizeVersion: accepts v-prefix and bare forms', () => {
  assert.deepEqual(normalizeVersion('v1.5.1'), { tag: 'v1.5.1', image: '1.5.1', parts: [1, 5, 1] });
  assert.deepEqual(normalizeVersion('1.5.1'), { tag: 'v1.5.1', image: '1.5.1', parts: [1, 5, 1] });
  assert.deepEqual(normalizeVersion('  v0.0.1 '), { tag: 'v0.0.1', image: '0.0.1', parts: [0, 0, 1] });
});

test('normalizeVersion: rejects garbage', () => {
  for (const bad of ['abc', '1.2', '1.2.3.4', 'v', '', null, undefined, 'archive/demo-data']) {
    assert.equal(normalizeVersion(bad), null, `expected null for ${bad}`);
  }
});

test('compareVersions: numeric field order', () => {
  const v = (s) => normalizeVersion(s);
  assert.ok(compareVersions(v('1.5.2'), v('1.5.1')) > 0);
  assert.ok(compareVersions(v('1.10.0'), v('1.9.9')) > 0);
  assert.ok(compareVersions(v('2.0.0'), v('1.99.99')) > 0);
  assert.equal(compareVersions(v('1.5.1'), v('v1.5.1')), 0);
});

test('bumpVersion: level increments and resets lower segments', () => {
  const v = normalizeVersion('1.2.3');
  assert.equal(bumpVersion(v, 'patch').image, '1.2.4');
  assert.equal(bumpVersion(v, 'minor').image, '1.3.0');
  assert.equal(bumpVersion(v, 'major').image, '2.0.0');
  assert.equal(bumpVersion(v, 'minor').tag, 'v1.3.0');
  assert.throws(() => bumpVersion(v, 'build'));
});

test('latestTag: picks highest v-semver, ignores non-version tags', () => {
  const output = [
    'archive/demo-data-fresh-db-register',
    'v1.0.0',
    'v1.5.1',
    'v1.5.0',
  ].join('\n');
  assert.equal(latestTag(output).tag, 'v1.5.1');
  assert.equal(latestTag(''), null);
  assert.equal(latestTag('not-a-version\nv2'), null);
});
