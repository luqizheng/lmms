// scripts/lib/VersionParser.mjs
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';

const isMain = import.meta.url === pathToFileURL(process.argv[1] ?? '').href;

export function parseVersion(raw) {
  if (typeof raw !== 'string') return null;
  const m = raw.match(/(\d+)\.(\d+)(?:\.(\d+))?/);
  if (!m) return null;
  return {
    major: Number(m[1]),
    minor: Number(m[2]),
    patch: m[3] === undefined ? 0 : Number(m[3]),
  };
}

export function compare(a, b) {
  if (a === null || b === null) return null;
  for (const k of ['major', 'minor', 'patch']) {
    if (a[k] !== b[k]) return a[k] < b[k] ? -1 : 1;
  }
  return 0;
}

export function isAtLeast(detected, required) {
  const c = compare(detected, required);
  return c === null ? false : c >= 0;
}

if (isMain) {
  assert.deepEqual(parseVersion('20.11.0'), { major: 20, minor: 11, patch: 0 });
  assert.deepEqual(parseVersion('Docker version 24.0.7, build afdd4d6'), { major: 24, minor: 0, patch: 7 });
  assert.deepEqual(parseVersion('rustc 1.78.0 (9b00956e3 2024-04-29)'), { major: 1, minor: 78, patch: 0 });
  assert.deepEqual(parseVersion('5.1.19041'), { major: 5, minor: 1, patch: 19041 });
  assert.deepEqual(parseVersion('garbage'), null);
  assert.equal(parseVersion(null), null);

  assert.equal(compare({ major: 20, minor: 0, patch: 0 }, { major: 18, minor: 0, patch: 0 }), 1);
  assert.equal(compare({ major: 17, minor: 0, patch: 0 }, { major: 18, minor: 0, patch: 0 }), -1);
  assert.equal(compare({ major: 18, minor: 0, patch: 0 }, { major: 18, minor: 0, patch: 0 }), 0);

  assert.equal(isAtLeast({ major: 20, minor: 0, patch: 0 }, { major: 18, minor: 0, patch: 0 }), true);
  assert.equal(isAtLeast({ major: 17, minor: 0, patch: 0 }, { major: 18, minor: 0, patch: 0 }), false);
  assert.equal(isAtLeast(null, { major: 18, minor: 0, patch: 0 }), false);

  console.log('VersionParser: all assertions passed');
}
