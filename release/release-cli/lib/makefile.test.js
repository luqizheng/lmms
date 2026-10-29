'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { readImageTag, applyVersion, bumpMakefileFile } = require('./makefile');

const SAMPLE = [
  '.PHONY: help secrets up down status logs verify load-images upgrade',
  '',
  'REGISTRY ?= zhcoder-docker-registry.com:8000',
  '# 镜像 tag 不带 v（git tag 带 v）',
  'IMAGE_TAG ?= 1.0.0',
  '',
  'help:',
  '\t@echo "coder-micro-services v1.0.0 — 可用命令："',
  '',
  'upgrade:',
  '\t@node bin/upgrade.js $(TAG)',
  '',
].join('\n');

test('readImageTag: extracts the default value', () => {
  assert.equal(readImageTag(SAMPLE), '1.0.0');
  assert.equal(readImageTag('IMAGE_TAG=9.9.9\n'), '9.9.9');
  assert.equal(readImageTag('no version here\n'), null);
});

test('applyVersion: rewrites IMAGE_TAG line and banner, preserves the rest', () => {
  const out = applyVersion(SAMPLE, '1.6.0');
  assert.match(out, /^IMAGE_TAG \?= 1\.6\.0$/m);
  assert.match(out, /coder-micro-services v1\.6\.0 — 可用命令：/);
  assert.match(out, /REGISTRY \?= zhcoder-docker-registry\.com:8000/);
  assert.match(out, /@node bin\/upgrade\.js \$\(TAG\)/);
  assert.equal(out.split('\n').length, SAMPLE.split('\n').length);
});

test('applyVersion: idempotent', () => {
  const once = applyVersion(SAMPLE, '1.6.0');
  assert.equal(applyVersion(once, '1.6.0'), once);
});

test('applyVersion: no IMAGE_TAG line still updates banner', () => {
  const bannerOnly = 'help:\n\t@echo "coder-micro-services v1.0.0 — x"\n';
  const out = applyVersion(bannerOnly, '2.0.0');
  assert.match(out, /coder-micro-services v2\.0\.0 — x/);
  assert.equal(readImageTag(out), null);
});

test('bumpMakefileFile: writes only when changed', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'cms-release-mk-'));
  const file = path.join(dir, 'Makefile');
  fs.writeFileSync(file, SAMPLE);

  assert.equal(bumpMakefileFile(file, '1.7.0'), true);
  assert.equal(readImageTag(fs.readFileSync(file, 'utf-8')), '1.7.0');

  assert.equal(bumpMakefileFile(file, '1.7.0'), false);
  fs.rmSync(dir, { recursive: true, force: true });
});
