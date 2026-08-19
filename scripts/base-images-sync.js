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
const { ROOT } = require('./lib/utils.js');
const { chalk } = require('./lib/ui.js');
const { scanProject } = require('./lib/Base-Images-Scanner.js');
const { mergeMirror, writeManifest, DEFAULT_BUILDER_REGISTRY, isBuilderRef, rewriteBuilderRef } = require('./lib/Base-Images-Writer.js');

const MANIFEST = path.join(ROOT, 'infrastructure', 'base-images.json');
const REGISTRY_CONFIG = path.join(ROOT, 'infrastructure', '.docker-registry.json');
const COMPOSE_ENV = path.join(ROOT, 'infrastructure', '.env');

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
      console.log(chalk.yellow(`[sync] warning: failed to parse ${REGISTRY_CONFIG}: ${err.message}`));
      console.log(chalk.yellow('[sync] falling back to defaults. Re-run `pnpm run setup` or delete the file.'));
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
      console.log(chalk.green('[sync] no changes'));
    } else {
      console.log(chalk.yellow('[sync] drift detected:'));
      console.log(`  old: ${oldJson}`);
      console.log(`  new: ${newJson}`);
      process.exit(2);
    }
    return;
  }
  writeManifest(MANIFEST, merged);
  syncComposeEnv(cfg.baseRegistry);
  const baseSrc = cfg.baseRegistry || '(docker hub)';
  console.log(chalk.green(`[sync] wrote ${merged.length} images to ${MANIFEST}`));
  console.log(`[sync] base registry:    ${baseSrc}`);
  console.log(`[sync] builder registry: ${cfg.builderRegistry}`);
}

// Mirror cfg.baseRegistry into infrastructure/.env as `BASE_REGISTRY=<value>/` so
// docker compose's ${BASE_REGISTRY}<image> interpolation matches the mirror we
// just wrote into base-images.json. Without this, re-running setup-wizard with
// a new baseRegistry would update .docker-registry.json but leave docker compose
// pulling from the previous registry.
//
//   - Empty baseRegistry => `BASE_REGISTRY=` (compose refs collapse to short
//     names — bare docker hub behavior).
//   - Trailing slash is always added so compose refs like
//     `${BASE_REGISTRY}mysql:8.0` concatenate without an extra slash.
//
// Idempotent: replaces the existing BASE_REGISTRY line in place, or appends a
// well-commented block if the line is missing. Other lines in .env are
// preserved byte-for-byte.
function syncComposeEnv(baseRegistry) {
  if (!fs.existsSync(COMPOSE_ENV)) {
    // No .env — nothing to do. docker compose will fail loudly on missing vars,
    // which is the correct signal for the user to run `pnpm run setup`.
    return;
  }
  const want = baseRegistry ? `${baseRegistry}/` : '';
  const lines = fs.readFileSync(COMPOSE_ENV, 'utf8').split('\n');
  // Allow `export BASE_REGISTRY=...` form (defensive — `.env` doesn't use it,
  // but if a user copy-pastes from a shell script it shouldn't break sync).
  const re = /^(export\s+)?(BASE_REGISTRY)\s*=\s*(.*?)\s*$/;
  let lineIdx = -1;
  let current = null;
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(re);
    if (m) {
      lineIdx = i;
      current = m[3];
      break;
    }
  }
  const has = (v) => v === want;
  if (lineIdx >= 0 && has(current)) return; // already in sync
  const newLine = `BASE_REGISTRY=${want}`;
  if (lineIdx >= 0) {
    lines[lineIdx] = newLine;
    fs.writeFileSync(COMPOSE_ENV, lines.join('\n'));
    console.log(chalk.dim(`[sync] updated infrastructure/.env: ${newLine}`));
  } else {
    // Append at end with a small header so future readers know where this came from.
    const sep = (lines.length && lines[lines.length - 1] !== '') ? '\n' : '';
    const block = `${sep}# BASE_REGISTRY — mirror for compose image refs (managed by pnpm run images:sync)\n${newLine}\n`;
    fs.writeFileSync(COMPOSE_ENV, lines.join('\n') + block);
    console.log(chalk.dim(`[sync] appended BASE_REGISTRY to infrastructure/.env`));
  }
}

function cmdSetMirror(ref, mirror) {
  const old = readOldManifest();
  const img = (old.images || []).find((i) => i.ref === ref);
  if (!img) {
    console.error(chalk.red(`[sync] unknown ref: ${ref}`));
    process.exit(1);
  }
  img.mirror = mirror || null;
  fs.writeFileSync(MANIFEST, JSON.stringify(old, null, 2) + '\n');
  console.log(chalk.green(`[sync] ${ref} mirror=${mirror || '(cleared)'}`));
}

function main() {
  const args = parseArgs(process.argv);
  if (args.command === 'set-mirror') {
    if (!args.ref || args.mirror === undefined) {
      console.error(chalk.red('usage: --set-mirror <ref> <mirror>'));
      process.exit(1);
    }
    cmdSetMirror(args.ref, args.mirror);
    return;
  }
  cmdSync(args.command === 'check');
}

main();