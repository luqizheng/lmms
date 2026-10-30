'use strict';

// Version parsing / comparing / bumping. Two conventions coexist in this repo:
// git tags carry the "v" prefix (v1.5.1); image tags and dist names do not
// (1.5.1). Every entry point accepts either form; helpers normalize internally.
// Ported from release-cli (main branch) — semantics unchanged.

const SEMVER_RE = /^v?(\d+)\.(\d+)\.(\d+)$/;

function normalizeVersion(input) {
  const m = SEMVER_RE.exec(String(input ?? '').trim());
  if (!m) return null;
  const parts = [Number(m[1]), Number(m[2]), Number(m[3])];
  return { tag: `v${parts.join('.')}`, image: parts.join('.'), parts };
}

function compareVersions(a, b) {
  for (let i = 0; i < 3; i++) {
    if (a.parts[i] !== b.parts[i]) return a.parts[i] - b.parts[i];
  }
  return 0;
}

// level: 'major' | 'minor' | 'patch'; lower segments reset to zero.
function bumpVersion(ver, level) {
  const idx = { major: 0, minor: 1, patch: 2 }[level];
  if (idx === undefined) throw new Error(`unknown bump level: ${level}`);
  const parts = ver.parts.slice();
  parts[idx] += 1;
  for (let i = idx + 1; i < 3; i++) parts[i] = 0;
  const image = parts.join('.');
  return { tag: `v${image}`, image, parts };
}

// Pick the highest v-semver tag from `git tag -l` output (one tag per line).
// Non-version tags (e.g. archive/...) are ignored.
function latestTag(gitTagOutput) {
  const versions = String(gitTagOutput || '')
    .split(/\r?\n/)
    .map((line) => normalizeVersion(line))
    .filter(Boolean)
    .sort(compareVersions);
  return versions.length ? versions[versions.length - 1] : null;
}

module.exports = { normalizeVersion, compareVersions, bumpVersion, latestTag };
