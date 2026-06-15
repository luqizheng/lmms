const test = require('node:test');
const assert = require('node:assert/strict');
const { mergeMirror, writeManifest } = require('../Base-Images-Writer.js');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');

test('mergeMirror keeps existing mirror on known refs', () => {
  const scanned = [
    { ref: 'redis:7-alpine', source: 'x', mirror: null },
    { ref: 'mysql:8.0', source: 'y', mirror: null },
  ];
  const old = { images: [{ ref: 'redis:7-alpine', source: 'old', mirror: 'docker.xunyuan.cn' }] };
  const merged = mergeMirror(scanned, old);
  const redis = merged.find((i) => i.ref === 'redis:7-alpine');
  assert.equal(redis.mirror, 'docker.xunyuan.cn');
  const mysql = merged.find((i) => i.ref === 'mysql:8.0');
  assert.equal(mysql.mirror, null);
});

test('writeManifest writes JSON with version + generatedAt', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'imgwrite-'));
  const out = path.join(tmp, 'base-images.json');
  writeManifest(out, [{ ref: 'alpine:3.21', source: 'a', mirror: null }]);
  const data = JSON.parse(fs.readFileSync(out, 'utf8'));
  assert.equal(data.version, 1);
  assert.ok(data.generatedAt);
  assert.equal(data.images.length, 1);
  fs.rmSync(tmp, { recursive: true, force: true });
});