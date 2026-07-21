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
const { mergeMirror, writeManifest, DEFAULT_BUILDER_REGISTRY, isBuilderRef, rewriteBuilderRef } = require('./lib/Base-Images-Writer.js');

const MANIFEST = path.join(ROOT, 'infrastructure', 'base-images.json');
const REGISTRY_CONFIG = path.join(ROOT, 'infrastructure', '.docker-registry.json');

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

// 读取 infrastructure/.docker-registry.json（由 scripts/setup-wizard.js 写入）。
// 文件不存在时回退到默认值（baseRegistry='', builderRegistry=DEFAULT_BUILDER_REGISTRY），
// 等价于 setup-wizard.js 未运行时的旧行为。
// 环境变量 CODER_BASE_REGISTRY / CODER_BUILDER_REGISTRY 优先于配置文件，便于 CI 覆盖。
function readRegistryConfig() {
  const cfg = { baseRegistry: '', builderRegistry: DEFAULT_BUILDER_REGISTRY };
  if (fs.existsSync(REGISTRY_CONFIG)) {
    try {
      const raw = JSON.parse(fs.readFileSync(REGISTRY_CONFIG, 'utf8'));
      if (raw.baseRegistry != null) cfg.baseRegistry = String(raw.baseRegistry);
      if (raw.builderRegistry != null) cfg.builderRegistry = String(raw.builderRegistry);
    } catch (err) {
      log.yellow(`[sync] warning: failed to parse ${REGISTRY_CONFIG}: ${err.message}`);
      log.yellow('[sync] falling back to defaults. Re-run `pnpm run setup` or delete the file.');
    }
  }
  if (process.env.CODER_BASE_REGISTRY != null) cfg.baseRegistry = process.env.CODER_BASE_REGISTRY;
  if (process.env.CODER_BUILDER_REGISTRY != null) cfg.builderRegistry = process.env.CODER_BUILDER_REGISTRY;
  return cfg;
}

function cmdSync(checkOnly) {
  const scanned = scanProject(ROOT);
  const old = readOldManifest();
  const cfg = readRegistryConfig();
  const merged = mergeMirror(scanned, old, cfg);
  merged.sort((a, b) => a.ref.localeCompare(b.ref));
  if (checkOnly) {
    // --check 比对原始 ref：builderRegistry 是本地配置（gitignored），
    // 不应让 CI 在开发者提交重写后的 manifest 时报 drift。把 builder ref
    // 还原成 Dockerfile 里写的原始形式（DEFAULT_BUILDER_REGISTRY 前缀）再比较。
    const normalizeRef = (ref) => rewriteBuilderRef(ref, DEFAULT_BUILDER_REGISTRY, cfg.builderRegistry);
    const oldJson = JSON.stringify((old.images || []).map((i) => normalizeRef(i.ref)).sort());
    const newJson = JSON.stringify(merged.map((i) => normalizeRef(i.ref)).sort());
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
  const baseSrc = cfg.baseRegistry || '(docker hub)';
  log.green(`[sync] wrote ${merged.length} images to ${MANIFEST}`);
  log.plain(`[sync] base registry:    ${baseSrc}`);
  log.plain(`[sync] builder registry: ${cfg.builderRegistry}`);
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