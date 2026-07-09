#!/usr/bin/env node
// scripts/images-pull.js
// 读取 infrastructure/base-images.json，按策略预拉每个基础镜像：
//   1) 若 mirror 字段非空，先尝试 docker pull <mirror>/<ref>（失败不回退，直接走第 2 步）
//   2) 无论上一步结果，跑 docker pull <ref>（已存在则 no-op）
//
// 用法：
//   node scripts/images-pull.js                # 实际拉镜像
//   node scripts/images-pull.js --dry-run      # 只打印计划，不真拉
//   pnpm run images:pull
//
// 退出码：0 全部成功；1 部分失败；2 manifest 缺失（需先跑 pnpm run images:sync）

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const MANIFEST = path.join(__dirname, '..', 'infrastructure', 'base-images.json');

function loadManifest() {
  const raw = fs.readFileSync(MANIFEST, 'utf8');
  const data = JSON.parse(raw);
  return Array.isArray(data.images) ? data.images : [];
}

function dockerPull(ref) {
  try {
    execFileSync('docker', ['pull', ref], { stdio: ['ignore', 'pipe', 'pipe'] });
    return { ok: true };
  } catch (err) {
    const stderr = err.stderr ? err.stderr.toString().trim() : err.message;
    const firstLine = stderr.split('\n').filter(Boolean).pop() || stderr;
    return { ok: false, reason: firstLine };
  }
}

function pullOne(img, dryRun) {
  const steps = [];
  if (img.mirror) {
    steps.push({ kind: 'mirror', ref: `${img.mirror}/${img.ref}` });
  }
  steps.push({ kind: 'source', ref: img.ref });
  const results = [];
  for (const step of steps) {
    if (dryRun) {
      results.push({ kind: step.kind, ref: step.ref, ok: true, dryRun: true });
      continue;
    }
    const r = dockerPull(step.ref);
    results.push({ kind: step.kind, ref: step.ref, ok: r.ok, reason: r.reason });
    if (!r.ok && step.kind === 'mirror') continue;
    if (!r.ok) break;
  }
  const sourceResult = results.find((r) => r.kind === 'source');
  return {
    ref: img.ref,
    ok: !!sourceResult && sourceResult.ok,
    steps: results,
  };
}

function main() {
  const dryRun = process.argv.includes('--dry-run');

  if (!fs.existsSync(MANIFEST)) {
    console.error(`manifest not found: ${MANIFEST}`);
    console.error('run `pnpm run images:sync` first.');
    process.exit(2);
  }

  const images = loadManifest();
  if (!images.length) {
    console.log('manifest has no images, nothing to pull.');
    return;
  }

  console.log(`${dryRun ? '[dry-run] ' : ''}pulling ${images.length} images from ${MANIFEST}`);

  const results = images.map((img, i) => {
    const idx = `[${String(i + 1).padStart(2, '0')}/${images.length}]`;
    const r = pullOne(img, dryRun);
    if (dryRun) {
      const flow = r.steps.map((s) => s.ref).join(' -> ');
      console.log(`${idx} ${img.ref}\n        plan: ${flow}`);
      return r;
    }
    process.stdout.write(`${idx} ${img.ref} ... `);
    if (r.ok) {
      process.stdout.write('OK\n');
    } else {
      const failedStep = r.steps.find((s) => !s.ok);
      process.stdout.write(`FAIL: ${failedStep ? failedStep.reason : 'unknown'}\n`);
    }
    return r;
  });

  const failed = results.filter((r) => !r.ok);
  console.log(`\nsummary: ${results.length - failed.length}/${results.length} succeeded`);
  if (failed.length) {
    console.log('failed refs:');
    for (const r of failed) console.log(`  - ${r.ref}`);
    process.exit(1);
  }
}

main();