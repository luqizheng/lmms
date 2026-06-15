#!/usr/bin/env node
// scripts/base-images-sync.js
// 扫描源码中的基础镜像，合并 mirror 字段，写入 infrastructure/base-images.json
//
// 用法：
//   node scripts/base-images-sync.js
//   node scripts/base-images-sync.js --check
//   node scripts/base-images-sync.js --set-mirror <ref> <mirror>

const fs = require('node:fs');
const path = require('node:path');
const { ROOT, log } = require('./lib/utils.js');
const { scanProject } = require('./lib/Base-Images-Scanner.js');
const { mergeMirror, writeManifest } = require('./lib/Base-Images-Writer.js');

const MANIFEST = path.join(ROOT, 'infrastructure', 'base-images.json');

function parseArgs(argv) {
  const out = { command: null };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--check') out.command = 'check';
    else if (a === '--set-mirror') {
      out.command = 'set-mirror';
      out.ref = argv[++i];
      out.mirror = argv[++i];
    }
  }
  return out;
}

function readOldManifest() {
  if (!fs.existsSync(MANIFEST)) return { images: [] };
  try { return JSON.parse(fs.readFileSync(MANIFEST, 'utf8')); }
  catch { return { images: [] }; }
}

function cmdSync(checkOnly) {
  const scanned = scanProject(ROOT);
  const old = readOldManifest();
  const merged = mergeMirror(scanned, old);
  merged.sort((a, b) => a.ref.localeCompare(b.ref));
  if (checkOnly) {
    const oldJson = JSON.stringify((old.images || []).map((i) => i.ref).sort());
    const newJson = JSON.stringify(merged.map((i) => i.ref).sort());
    if (oldJson === newJson) {
      log.green('[sync] no changes');
    } else {
      log.yellow('[sync] drift detected:');
      log.plain(`  old: ${oldJson}`);
      log.plain(`  new: ${newJson}`);
      process.exit(2);
    }
    return;
  }
  writeManifest(MANIFEST, merged);
  log.green(`[sync] wrote ${merged.length} images to ${MANIFEST}`);
}

function cmdSetMirror(ref, mirror) {
  const old = readOldManifest();
  const img = (old.images || []).find((i) => i.ref === ref);
  if (!img) {
    log.red(`[sync] unknown ref: ${ref}`);
    process.exit(1);
  }
  img.mirror = mirror || null;
  fs.writeFileSync(MANIFEST, JSON.stringify(old, null, 2) + '\n');
  log.green(`[sync] ${ref} mirror=${mirror || '(cleared)'}`);
}

function main() {
  const args = parseArgs(process.argv);
  if (args.command === 'set-mirror') {
    if (!args.ref || args.mirror === undefined) {
      log.red('usage: --set-mirror <ref> <mirror>');
      process.exit(1);
    }
    cmdSetMirror(args.ref, args.mirror);
    return;
  }
  cmdSync(args.command === 'check');
}

main();