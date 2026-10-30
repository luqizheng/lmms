'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { LIBS_ORDER, SDKS_ORDER, rewritePathDeps, publishRust } = require('../lib/rust-publish.js');
const { ExternalError } = require('../lib/errors.js');

const TOML_SAMPLE = [
  '[package]',
  'name = "digua_common"',
  'version = "1.7.0"',
  '',
  '[dependencies]',
  '# internal deps stay path-based in the repo',
  'digua_common_error = { path = "../common-error" }',
  'digua_common_db = { path = "../common-db", features = ["mysql"] }',
  'serde = "1"',
].join('\n');

function crateToml(name) {
  return [
    '[package]',
    `name = "${name}"`,
    'version = "1.7.0"',
    'description = "test crate"',
    'license = "MIT"',
    '',
    '[dependencies]',
    'digua_common_error = { path = "../common-error" }',
  ].join('\n');
}

function fixture({ dirs }) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-rustpub-'));
  for (const dir of dirs) {
    const d = path.join(root, 'libs', dir);
    fs.mkdirSync(d, { recursive: true });
    fs.writeFileSync(path.join(d, 'Cargo.toml'), crateToml('digua_' + dir.replaceAll('-', '_')), 'utf-8');
  }
  return root;
}

test('rewritePathDeps swaps path deps for version+registry, keeps the rest', () => {
  const out = rewritePathDeps(TOML_SAMPLE, { crateVersions: { 'common-error': '1.7.0', 'common-db': '1.7.0' } });
  assert.ok(out.includes('digua_common_error = { version = "1.7.0", registry = "kellnr" }'));
  assert.ok(out.includes('digua_common_db = { version = "1.7.0", registry = "kellnr", features = ["mysql"] }'));
  assert.ok(out.includes('# internal deps stay path-based in the repo'), 'comment line untouched');
  assert.ok(out.includes('name = "digua_common"'), 'name line untouched');
  assert.ok(out.includes('serde = "1"'), 'external dep untouched');
  assert.ok(!out.includes('path ='), 'no path dep remains');
});

test('topology constants: common-error before common; http-client first among sdks', () => {
  assert.ok(LIBS_ORDER.indexOf('common-error') < LIBS_ORDER.indexOf('common'));
  assert.equal(LIBS_ORDER.length, 17);
  assert.equal(SDKS_ORDER[0], 'digua-http-client');
  assert.equal(SDKS_ORDER.length, 7);
});

test('rollback: failure at the third crate restores every rewritten Cargo.toml', () => {
  const root = fixture({ dirs: LIBS_ORDER });
  const writes = [];
  const realWrite = fs.writeFileSync.bind(fs);
  const fsSpy = {
    ...fs,
    writeFileSync: (p, data, opts) => { writes.push([String(p), String(data)]); realWrite(p, data, opts); },
  };
  const exec = (file, args) => {
    if (file === 'cargo' && args.includes('digua_jwt_ctx')) return { status: 5, stderr: 'boom' };
    return { status: 0 };
  };

  assert.throws(
    () => publishRust({ root, version: '1.7.0', targets: 'libs', exec, fs: fsSpy, out: { log() {}, warn() {} } }),
    (err) => {
      assert.ok(err instanceof ExternalError);
      assert.equal(err.exitCode, 4);
      return true;
    },
  );

  const tomlOf = (dir) => path.join(root, 'libs', dir, 'Cargo.toml');
  const original = crateToml('digua_common_health');
  const restoreWrites = writes.filter(([p]) => p === tomlOf('common-health'));
  assert.ok(restoreWrites.length >= 2, 'rewritten once, then restored');
  assert.equal(restoreWrites[restoreWrites.length - 1][1], original, 'final content is the original text');
  const left = fs.readFileSync(tomlOf('common-error'), 'utf-8');
  assert.equal(left, crateToml('digua_common_error'), 'first crate restored too');
});

test('dryRun: zero exec calls, plan returned, no file modification', () => {
  const root = fixture({ dirs: ['common-error', 'common-health'] });
  const before = fs.readFileSync(path.join(root, 'libs', 'common-error', 'Cargo.toml'), 'utf-8');
  let execCalls = 0;
  const result = publishRust({
    root,
    version: '1.7.0',
    targets: 'libs',
    dryRun: true,
    exec: () => { execCalls += 1; return { status: 0 }; },
    out: { log() {}, warn() {} },
  });
  assert.equal(execCalls, 0);
  assert.deepEqual(result.published, []);
  assert.ok(result.plan.length >= 2, 'plan lists the crates');
  const after = fs.readFileSync(path.join(root, 'libs', 'common-error', 'Cargo.toml'), 'utf-8');
  assert.equal(after, before, 'no rewrite on dry-run');
});
