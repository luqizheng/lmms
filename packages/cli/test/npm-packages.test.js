'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { PACKAGES, REGISTRY, checkPackages, publishPackages } = require('../lib/npm-packages.js');
const { GuardError } = require('../lib/errors.js');

function fixture({ dists = true, versions = '0.1.0' } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-npmpkg-'));
  for (const pkg of PACKAGES) {
    const dir = path.join(root, pkg.dir);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'package.json'), JSON.stringify({
      name: pkg.name,
      version: versions,
      private: false,
      publishConfig: { registry: REGISTRY },
    }), 'utf-8');
    if (dists) {
      fs.mkdirSync(path.join(dir, 'dist'), { recursive: true });
      fs.writeFileSync(path.join(dir, 'dist', 'index.js'), 'export {};', 'utf-8');
    }
  }
  return root;
}

const cleanExec = () => ({ status: 0, stdout: '' });
const okHttp = async () => 200;

test('PACKAGES covers the three @digua packages plus the six service SDKs', () => {
  // Expectations are derived from the manifest, not hardcoded: the set grows
  // whenever a package joins the publish pipeline (6 SDKs joined in 4ad4095a).
  const names = PACKAGES.map((p) => p.name);
  assert.ok(names.includes('@digua/contracts'));
  assert.ok(names.includes('@digua/http'));
  assert.ok(names.includes('@digua/cli'));
  for (const sdk of ['iam-services', 'audit-services', 'file-services', 'notification-services', 'prue-services']) {
    assert.ok(names.includes(sdk), `PACKAGES must include ${sdk}`);
  }
  // @digua/cli is pure CJS — it needs no build step.
  assert.equal(PACKAGES.find((p) => p.name === '@digua/cli').needsBuild, false);
  assert.equal(PACKAGES.find((p) => p.name === '@digua/contracts').needsBuild, true);
});

test('every PACKAGES entry points at a distinct directory', () => {
  const dirs = PACKAGES.map((p) => p.dir);
  assert.equal(new Set(dirs).size, dirs.length, 'duplicate package dir in PACKAGES');
});

test('an unreachable registry fails the check with a registry item', async () => {
  const root = fixture();
  const result = await checkPackages({
    root,
    nextVersion: '0.2.0',
    httpGet: async () => { throw new Error('ECONNREFUSED'); },
    exec: cleanExec,
  });
  assert.equal(result.ok, false);
  assert.ok(result.failures.some((f) => f.includes('registry')), JSON.stringify(result.failures));
});

test('v-prefixed nextVersion is normalized for the version-order check', async () => {
  const rootLower = fixture({ versions: '0.1.0' });
  const pass = await checkPackages({ root: rootLower, nextVersion: 'v0.2.0', httpGet: okHttp, exec: cleanExec });
  assert.equal(pass.ok, true, JSON.stringify(pass.failures));

  const rootSame = fixture({ versions: '0.2.0' });
  const fail = await checkPackages({ root: rootSame, nextVersion: 'v0.2.0', httpGet: okHttp, exec: cleanExec });
  assert.equal(fail.ok, false);
  assert.ok(fail.failures.some((f) => f.includes('低于目标版本')), 'same version counts as not-lower');
});

test('needsBuild:false skips the dist artifact check', async () => {
  const root = fixture({ dists: false });
  const result = await checkPackages({ root, nextVersion: '0.2.0', httpGet: okHttp, exec: cleanExec });
  assert.equal(result.ok, false, 'packages needing a build still lack dist');
  const cliMissing = result.failures.filter((f) => f.includes('@digua/cli'));
  assert.deepEqual(cliMissing, [], 'cli is not required to have dist');
});

test('publishPackages: build first, versions and publishes last, in PACKAGES order', async () => {
  const root = fixture();
  const calls = [];
  const exec = (file, args, opts = {}) => {
    calls.push([file, ...args, opts.cwd ?? opts.prefix ?? '']);
    return { status: 0, stdout: '' };
  };
  const result = await publishPackages({
    root,
    version: '0.2.0',
    exec,
    check: async () => ({ ok: true, failures: [], notes: [] }),
    out: { log() {}, warn() {} },
  });
  assert.deepEqual(result.published.map((p) => p.name), PACKAGES.map((p) => p.name));
  assert.equal(calls[0][0] + calls[0][1], 'pnpmrun', 'build runs first');
  assert.ok(calls[0].includes('packages:build'));
  const versions = calls.filter((c) => c.includes('version') && c[0] === 'npm');
  const publishes = calls.filter((c) => c.includes('publish') && c[0] === 'npm');
  assert.equal(versions.length, PACKAGES.length);
  assert.equal(publishes.length, PACKAGES.length);
  const lastIdx = calls.length - 1;
  assert.equal(calls[lastIdx].includes('publish'), true, 'publish runs last');
  // publish runs with cwd set to the package directory — that is what proves
  // the ordering, not the positional index of the flag list.
  assert.deepEqual(
    publishes.map((c) => c[c.length - 1]),
    PACKAGES.map((p) => path.join(root, p.dir)),
    'publish order follows PACKAGES order',
  );
});

test('publishPackages: a failed check stops everything after build', async () => {
  const root = fixture();
  const calls = [];
  const exec = (file, args) => { calls.push([file, ...args]); return { status: 0, stdout: '' }; };
  await assert.rejects(
    () => publishPackages({
      root,
      version: '0.2.0',
      exec,
      check: async () => ({ ok: false, failures: ['x'], notes: [] }),
      out: { log() {}, warn() {} },
    }),
    (err) => {
      assert.ok(err instanceof GuardError);
      assert.equal(err.exitCode, 3);
      return true;
    },
  );
  assert.equal(calls.length, 1, 'only the build ran');
});

test('pnpm/npm invocations carry the Windows shell shim on win32', async () => {
  const root = fixture();
  const optsSeen = [];
  const exec = (file, args, opts = {}) => { optsSeen.push([file, Boolean(opts.shell)]); return { status: 0, stdout: '' }; };
  await publishPackages({
    root,
    version: '0.2.0',
    exec,
    check: async () => ({ ok: true, failures: [], notes: [] }),
    out: { log() {}, warn() {} },
  });
  const expected = process.platform === 'win32';
  for (const [file, shelled] of optsSeen) {
    if (file === 'pnpm' || file === 'npm') {
      assert.equal(shelled, expected, `${file} exec opts.shell must be ${expected} on ${process.platform}`);
    }
  }
  assert.ok(optsSeen.some(([f]) => f === 'pnpm'));
  assert.ok(optsSeen.some(([f]) => f === 'npm'));
});
