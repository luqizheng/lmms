'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { readTopVersion, hasVersion, insertEntry } = require('../lib/changelog.js');

const DOC = '# Changelog\n\n## v1.0.0 (2026-07-12)\n\n### Added\n- 初始版本\n';

test('reads top version and tests membership', () => {
  assert.equal(readTopVersion(DOC), '1.0.0');
  assert.equal(hasVersion(DOC, '1.0.0'), true);
  assert.equal(hasVersion(DOC, '1.7.0'), false);
});

test('returns null when no version entries exist', () => {
  assert.equal(readTopVersion('# Changelog\n\nsome notes\n'), null);
});

test('inserts new entry above existing ones', () => {
  const out = insertEntry(DOC, { version: '1.7.0', date: '2026-10-30', body: '### Added\n- coder CLI' });
  assert.equal(readTopVersion(out), '1.7.0');
  assert.ok(out.startsWith('# Changelog\n\n## v1.7.0 (2026-10-30)\n\n### Added\n- coder CLI\n\n## v1.0.0'));
});

test('inserts into a document without prior entries', () => {
  const out = insertEntry('# Changelog\n', { version: '1.7.0', date: '2026-10-30', body: '- first' });
  assert.equal(readTopVersion(out), '1.7.0');
  assert.ok(out.startsWith('# Changelog\n\n## v1.7.0 (2026-10-30)\n\n- first'));
});

test('keeps CRLF documents CRLF', () => {
  const out = insertEntry(DOC.replaceAll('\n', '\r\n'), { version: '1.7.0', date: '2026-10-30', body: '- x' });
  assert.ok(!/[^\r]\n/.test(out));
});
