'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { SWAGGER, verifySha256, ensureAsset } = require('../lib/assets.js');
const { CoderError } = require('../lib/errors.js');

const GOOD = Buffer.from('abc');
const GOOD_HEX = crypto.createHash('sha256').update(GOOD).digest('hex');

function fixture({ withFile = false } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-assets-'));
  const dest = path.join(dir, 'infrastructure', 'vendor', SWAGGER.ZIP_NAME);
  if (withFile) {
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    fs.writeFileSync(dest, GOOD);
  }
  return dir;
}

test('swagger metadata matches the retired script verbatim', () => {
  assert.equal(SWAGGER.VERSION, 'v5.17.14');
  assert.equal(SWAGGER.ZIP_NAME, 'swagger-ui-v5.17.14.zip');
  assert.equal(SWAGGER.SHA256, '481244d0812097b11fbaeef79f71d942b171617f9c9f9514e63acbe13e71ccdc');
});

test('verifySha256 accepts the correct digest and rejects a wrong one', () => {
  assert.equal(verifySha256(GOOD, GOOD_HEX), true);
  assert.equal(verifySha256(GOOD, '00'.repeat(32)), false);
});

test('ensure keeps an existing file without fetching', async () => {
  const dir = fixture({ withFile: true });
  let fetches = 0;
  const result = await ensureAsset({ root: dir, mode: 'ensure', fetchImpl: () => { fetches += 1; return GOOD; } });
  assert.equal(result.existed, true);
  assert.equal(result.downloaded, false);
  assert.equal(fetches, 0);
  assert.equal(result.path, path.join(dir, 'infrastructure', 'vendor', SWAGGER.ZIP_NAME));
});

test('check mode throws ASSET_MISSING (exit 3) when the file is absent', async () => {
  const dir = fixture();
  await assert.rejects(
    () => ensureAsset({ root: dir, mode: 'check', fetchImpl: () => GOOD }),
    (err) => {
      assert.ok(err instanceof CoderError);
      assert.equal(err.exitCode, 3);
      assert.equal(err.code, 'ASSET_MISSING');
      return true;
    },
  );
});

test('a downloaded payload with a bad sha256 is rejected and never written', async () => {
  const dir = fixture();
  await assert.rejects(
    () => ensureAsset({ root: dir, mode: 'force', fetchImpl: () => Buffer.from('tampered') }),
    (err) => {
      assert.ok(err instanceof CoderError);
      assert.equal(err.code, 'ASSET_SHA_MISMATCH');
      return true;
    },
  );
  const dest = path.join(dir, 'infrastructure', 'vendor', SWAGGER.ZIP_NAME);
  assert.equal(fs.existsSync(dest), false, 'no file on disk after sha mismatch');
});

test('force mode re-downloads and verifies a good payload', async () => {
  const dir = fixture({ withFile: true });
  let fetches = 0;
  const result = await ensureAsset({ root: dir, mode: 'force', fetchImpl: () => { fetches += 1; return GOOD; } });
  assert.equal(fetches, 1);
  assert.equal(result.downloaded, true);
});
