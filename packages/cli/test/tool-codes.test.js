'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '../../..');
const TOOL_CODES_PATH = path.join(repoRoot, 'contracts', 'tool-codes.yaml');
const GENERATED_PATH = path.join(repoRoot, 'packages', 'cli', 'lib', 'tool-codes.js');

// Codes the CLI raises today (grep-verified 2026-11-02). The YAML must cover
// all of them so the code table stays the single source of truth.
const EXISTING_CODES = [
  'ASSET_MISSING', 'ASSET_SHA_MISMATCH',
  'CRATE_FIELD_MISSING', 'CRATE_TOML_MISSING', 'CRATE_VERSION_MISMATCH',
  'DIRTY_WORKTREE',
  'MAVEN_BINARY_MISSING', 'MAVEN_CREDENTIALS_MISSING', 'MAVEN_CREDENTIALS_PARTIAL',
  'MAVEN_NO_ARTIFACTS', 'MAVEN_POM_MALFORMED', 'MAVEN_POM_MISSING',
  'MAVEN_PROFILE_INVALID', 'MAVEN_VERSION_NOT_SNAPSHOT', 'MAVEN_VERSION_REQUIRED',
  'MAVEN_VERSION_SNAPSHOT',
  'NO_BASELINE',
  'NPM_BUILD_FAILED', 'NPM_PACKAGES_CHECK_FAILED', 'NPM_PUBLISH_FAILED',
  'NPM_VERSION_FAILED', 'NPM_VERSION_REQUIRED',
  'RUST_CARGO_MISSING', 'RUST_VERSION_REQUIRED',
  'TAG_EXISTS', 'VERSION_DRIFT',
];

const NEW_CODES = [
  'UNKNOWN_COMMAND', 'UNKNOWN_FLAG', 'MISSING_FLAG_VALUE', 'NO_REPO_CONTEXT',
  'MISSING_POSITIONAL', 'TOO_MANY_POSITIONAL',
];

function loadYaml() {
  const fs = require('node:fs');
  const content = fs.readFileSync(TOOL_CODES_PATH, 'utf-8');
  // Minimal parser: `KEY:` blocks with `field: value` pairs.
  const codes = {};
  let current = null;
  for (const raw of content.split('\n')) {
    const line = raw.replace(/\s+#.*$/, '').trimEnd();
    if (!line.trim() || line.trim().startsWith('#')) continue;
    const m = line.match(/^  ([A-Z_0-9]+):\s*$/);
    if (m) { current = m[1]; codes[current] = {}; continue; }
    const f = line.match(/^    ([a-z]+):\s*(.*)$/);
    if (f && current) codes[current][f[1]] = f[2].trim().replace(/^"|"$/g, '');
  }
  return codes;
}

test('tool-codes.yaml exists and declares every code the CLI raises', () => {
  const codes = loadYaml();
  for (const code of [...EXISTING_CODES, ...NEW_CODES]) {
    assert.ok(codes[code], `tool-codes.yaml must declare ${code}`);
  }
});

test('every tool code declares message, kind and exit', () => {
  const codes = loadYaml();
  for (const [code, def] of Object.entries(codes)) {
    assert.ok(def.message, `${code} needs a message`);
    assert.ok(def.kind, `${code} needs a kind`);
    assert.ok(def.exit !== undefined && def.exit !== '', `${code} needs an exit code`);
  }
});

test('tool code kinds map onto the CLI exit-code contract', () => {
  const codes = loadYaml();
  const kindToExit = { Usage: 2, Guard: 3, External: 4, Failure: 1 };
  for (const [code, def] of Object.entries(codes)) {
    assert.equal(
      Number(def.exit),
      kindToExit[def.kind],
      `${code} (kind ${def.kind}) must use exit ${kindToExit[def.kind]}`,
    );
  }
});

test('RUST_VERSION_REQUIRED is declared exactly once, as a usage error', () => {
  // The same code was previously raised as both UsageError and GuardError,
  // giving it two possible exit codes. Usage wins: the version argument is
  // missing, which is a usage problem, not a state problem.
  const codes = loadYaml();
  assert.equal(codes.RUST_VERSION_REQUIRED.kind, 'Usage');
  assert.equal(Number(codes.RUST_VERSION_REQUIRED.exit), 2);
});

test('generated tool-codes.js is in sync with the YAML', () => {
  const fs = require('node:fs');
  const generated = fs.readFileSync(GENERATED_PATH, 'utf-8');
  const codes = loadYaml();
  for (const code of Object.keys(codes)) {
    assert.ok(generated.includes(code), `generated file must contain ${code}`);
  }
  assert.match(generated, /DO NOT EDIT MANUALLY/);
});

test('generated module exports TOOL_CODES keyed by code with exit numbers', () => {
  const { TOOL_CODES } = require(GENERATED_PATH);
  assert.equal(typeof TOOL_CODES, 'object');
  assert.equal(TOOL_CODES.VERSION_DRIFT.kind, 'Guard');
  assert.equal(TOOL_CODES.VERSION_DRIFT.exit, 3);
  assert.equal(TOOL_CODES.UNKNOWN_FLAG.exit, 2);
});

test('every error code raised anywhere in the CLI is declared in the table', () => {
  // Guards against orphan codes: a code thrown in the source but missing from
  // the YAML would leave an agent with no contract to look up.
  const fs = require('node:fs');
  const cliDir = path.join(repoRoot, 'packages', 'cli');
  const codes = loadYaml();
  const found = new Set();
  const scan = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) { if (entry.name !== 'test') scan(full); continue; }
      if (!entry.name.endsWith('.js')) continue;
      const content = fs.readFileSync(full, 'utf-8');
      for (const m of content.matchAll(/new (?:UsageError|GuardError|ExternalError)\('([A-Z_0-9]+)'/g)) {
        found.add(m[1]);
      }
    }
  };
  scan(cliDir);
  const orphans = [...found].filter((c) => !codes[c]);
  assert.deepEqual(orphans, [], `undeclared error codes: ${orphans.join(', ')}`);
});
