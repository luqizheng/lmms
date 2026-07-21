#!/usr/bin/env node
/**
 * scripts/setup-wizard.js
 *
 * 交互式配置 Docker 镜像拉取来源 + Rust crates.io 镜像与 token。
 *
 * 输出文件（均 gitignored，本地配置不入 git）：
 *   - infrastructure/.docker-registry.json
 *       baseRegistry / builderRegistry，由 scripts/base-images-sync.js 读取。
 *   - .cargo/config.toml
 *       [source.crates-io] replace-with + [source.coder-mirror] / [source.kellnr] / [net]。
 *       会被覆盖式重写（保留 kellnr 私有 registry 与 git-fetch-with-cli）。
 *   - .cargo/credentials.toml
 *       [registry.<name>] token = "..."（仅当 crateToken 非空时生成）。
 *
 * 两类 Docker 镜像：
 *   1. 基础镜像（rabbitmq / redis / nginx / mysql / consul / grafana 等）
 *      - 留空 = Docker Hub（hub.docker.com），mirror=null
 *      - 输入 registry URL = 作为 mirror，images-pull 会先尝试 `<baseRegistry>/<ref>`
 *   2. Builder 镜像（rust / java / maven / node 等编译环境）
 *      - 默认 zhcoder-docker-registry.com:8000
 *      - 输入其他 registry = 重写 ref 的 registry 前缀
 *
 * crates.io 镜像：
 *   - 留空 = 官方 crates.io（移除 replace-with）
 *   - 输入 URL = 作为 [source.coder-mirror] 替换 crates-io。
 *     自动补 sparse+ 前缀；若只给 host:port（如 http://192.168.2.12:16444），
 *     自动补 /api/v1/crates/（kellnr 默认路径）。
 *   - token：留空 = 不写 credentials.toml；非空 = 写入 [registry.<name>] token。
 *
 * 优先级（高 -> 低）：CLI 参数 > 环境变量 > 现有配置文件 > 默认值
 * 非交互模式：stdin 不是 TTY，或传 --non-interactive / --defaults 时启用。
 *
 * 用法：
 *   node scripts/setup-wizard.js                 # 交互式（默认）
 *   node scripts/setup-wizard.js --non-interactive
 *   node scripts/setup-wizard.js --defaults      # 强制用默认值，跳过交互
 *   node scripts/setup-wizard.js --show          # 打印当前配置后退出
 *   CODER_BASE_REGISTRY=... CODER_BUILDER_REGISTRY=... \
 *     CODER_CRATE_REGISTRY=... CODER_CRATE_TOKEN=... node scripts/setup-wizard.js --non-interactive
 */

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const readline = require('node:readline/promises');
const { stdin, stdout } = require('node:process');

const { ROOT, log } = require('./lib/utils.js');
const { DEFAULT_BUILDER_REGISTRY } = require('./lib/Base-Images-Writer.js');

const CONFIG_PATH = path.join(ROOT, 'infrastructure', '.docker-registry.json');
const CARGO_DIR = path.join(ROOT, '.cargo');
const CARGO_CONFIG_PATH = path.join(CARGO_DIR, 'config.toml');
const CARGO_CREDENTIALS_PATH = path.join(CARGO_DIR, 'credentials.toml');

// 项目共享的私有 kellnr registry（libs/* 和 sdk/rust/* 发布到此处）。
// wizard 重生成 .cargo/config.toml 时始终保留这一段。
const KELLNR_REGISTRY = 'sparse+http://192.168.2.12:16444/api/v1/crates/';
// crates.io 镜像替换后的 source 名字（[source.coder-mirror]）。
const MIRROR_NAME = 'coder-mirror';
// 交互提示用的 example URL（与 kellnr 同 host，便于团队识别）。
const CRATE_REGISTRY_EXAMPLE = 'http://192.168.2.12:16444';

const DEFAULTS = {
  baseRegistry: '',
  builderRegistry: DEFAULT_BUILDER_REGISTRY,
  crateRegistry: '',
  crateToken: '',
};

function parseArgs(argv) {
  const out = { nonInteractive: false, defaults: false, show: false };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--non-interactive' || a === '--no-interaction') out.nonInteractive = true;
    else if (a === '--defaults') out.defaults = true;
    else if (a === '--show') out.show = true;
    else if (a === '-h' || a === '--help') {
      printHelp();
      process.exit(0);
    } else {
      log.red(`unknown argument: ${a}`);
      printHelp();
      process.exit(1);
    }
  }
  return out;
}

function printHelp() {
  console.log('usage: node scripts/setup-wizard.js [--non-interactive|--defaults|--show]');
  console.log('');
  console.log('  --non-interactive   skip prompts; use env vars or existing config or defaults');
  console.log('  --defaults          force defaults (base=docker hub, builder=zhcoder-docker-registry.com:8000,');
  console.log('                      crates.io=official, no token)');
  console.log('  --show              print current config and exit');
  console.log('');
  console.log('env vars (override config file):');
  console.log('  CODER_BASE_REGISTRY      base images registry (empty = docker hub)');
  console.log('  CODER_BUILDER_REGISTRY   builder images registry');
  console.log('  CODER_CRATE_REGISTRY     crates.io mirror URL (empty = official crates.io)');
  console.log('  CODER_CRATE_TOKEN        crates.io / mirror auth token');
}

function readExistingConfig() {
  if (!fs.existsSync(CONFIG_PATH)) return null;
  try {
    const raw = JSON.parse(fs.readFileSync(CONFIG_PATH, 'utf8'));
    return {
      baseRegistry: raw.baseRegistry != null ? String(raw.baseRegistry) : '',
      builderRegistry: raw.builderRegistry != null ? String(raw.builderRegistry) : DEFAULT_BUILDER_REGISTRY,
      crateRegistry: raw.crateRegistry != null ? String(raw.crateRegistry) : '',
      crateToken: raw.crateToken != null ? String(raw.crateToken) : '',
      updatedAt: raw.updatedAt || null,
    };
  } catch (err) {
    log.yellow(`[wizard] warning: failed to parse ${CONFIG_PATH}: ${err.message}`);
    return null;
  }
}

function writeConfig(cfg) {
  const data = {
    version: 2,
    baseRegistry: cfg.baseRegistry,
    builderRegistry: cfg.builderRegistry,
    crateRegistry: cfg.crateRegistry,
    crateToken: cfg.crateToken,
    updatedAt: new Date().toISOString(),
  };
  fs.mkdirSync(path.dirname(CONFIG_PATH), { recursive: true });
  fs.writeFileSync(CONFIG_PATH, JSON.stringify(data, null, 2) + '\n');
}

function showSummary(cfg, source) {
  const baseDisplay = cfg.baseRegistry || '(docker hub)';
  const crateDisplay = cfg.crateRegistry || '(official crates.io)';
  const tokenDisplay = cfg.crateToken ? '(set)' : '(none)';
  log.green(`[wizard] config written to ${path.relative(ROOT, CONFIG_PATH)}`);
  log.plain(`[wizard] source:            ${source}`);
  log.plain(`[wizard] base registry:     ${baseDisplay}`);
  log.plain(`[wizard] builder registry:  ${cfg.builderRegistry}`);
  log.plain(`[wizard] crates.io mirror:  ${crateDisplay}`);
  log.plain(`[wizard] crates.io token:   ${tokenDisplay}`);
  log.plain('');
  log.plain('[wizard] next: `pnpm run images:sync` will apply these to base-images.json');
  log.plain('[wizard]       .cargo/config.toml regenerated (kellnr + [net] preserved)');
  if (cfg.crateToken) {
    log.plain('');
    log.yellow('[wizard] token written to .cargo/credentials.toml (gitignored).');
    log.plain('[wizard] cargo 默认不读项目级 credentials.toml；应用 token 请执行：');
    log.plain(`[wizard]   export CARGO_CREDENTIALS_PATH="${path.relative(ROOT, CARGO_CREDENTIALS_PATH)}"`);
    log.plain('[wizard] 或将该 export 加入 shell 配置（~/.bashrc / ~/.zshrc）。');
  } else if (fs.existsSync(CARGO_CREDENTIALS_PATH)) {
    log.plain('');
    log.plain('[wizard] .cargo/credentials.toml removed (token cleared).');
  }
}

function showCurrent() {
  const existing = readExistingConfig();
  if (!existing) {
    log.yellow(`[wizard] no config at ${path.relative(ROOT, CONFIG_PATH)}`);
    log.plain('[wizard] defaults would be:');
    log.plain(`  base registry:     (docker hub)`);
    log.plain(`  builder registry: ${DEFAULTS.builderRegistry}`);
    log.plain(`  crates.io mirror: (official)`);
    log.plain(`  crates.io token:  (none)`);
    return;
  }
  log.green(`[wizard] current config at ${path.relative(ROOT, CONFIG_PATH)}`);
  log.plain(`  base registry:     ${existing.baseRegistry || '(docker hub)'}`);
  log.plain(`  builder registry:  ${existing.builderRegistry}`);
  log.plain(`  crates.io mirror:  ${existing.crateRegistry || '(official crates.io)'}`);
  log.plain(`  crates.io token:   ${existing.crateToken ? '(set)' : '(none)'}`);
  log.plain(`  updatedAt:         ${existing.updatedAt || '(unknown)'}`);
}

function validateBuilderRegistry(value) {
  const v = (value || '').trim();
  if (!v) return { ok: false, reason: 'builder registry cannot be empty' };
  if (/\s/.test(v)) return { ok: false, reason: 'must not contain whitespace' };
  return { ok: true, value: v };
}

function validateBaseRegistry(value) {
  const v = (value || '').trim();
  if (v && /\s/.test(v)) return { ok: false, reason: 'must not contain whitespace' };
  return { ok: true, value: v };
}

// 接受 base URL（如 http://192.168.2.12:16444）或完整 cargo registry URL
// （如 sparse+https://rsproxy.cn/index/）。留空 = 官方 crates.io。
// 自动补 sparse+ 前缀；若 URL 只有 host:port（无路径），补 kellnr 默认路径。
function normalizeCrateRegistry(value) {
  const v = (value || '').trim();
  if (!v) return '';
  let url = v;
  if (!url.startsWith('sparse+')) {
    url = 'sparse+' + url;
  }
  const afterSparse = url.slice('sparse+'.length);
  if (/^https?:\/\/[^/]+\/?$/.test(afterSparse)) {
    url = 'sparse+' + afterSparse.replace(/\/$/, '') + '/api/v1/crates/';
  }
  return url;
}

function validateCrateRegistry(value) {
  const v = (value || '').trim();
  if (v && /\s/.test(v)) return { ok: false, reason: 'must not contain whitespace' };
  return { ok: true, value: v };
}

function validateCrateToken(value) {
  const v = (value || '').trim();
  if (v && /\s/.test(v)) return { ok: false, reason: 'must not contain whitespace' };
  return { ok: true, value: v };
}

function resolveNonInteractive(args, existing) {
  // CLI --defaults > env vars > existing config > defaults
  if (args.defaults) return { cfg: { ...DEFAULTS }, source: 'defaults (forced)' };

  const cfg = { ...DEFAULTS };
  let source = 'defaults';

  if (existing) {
    cfg.baseRegistry = existing.baseRegistry;
    cfg.builderRegistry = existing.builderRegistry;
    cfg.crateRegistry = existing.crateRegistry;
    cfg.crateToken = existing.crateToken;
    source = 'existing config';
  }

  if (process.env.CODER_BASE_REGISTRY != null) {
    cfg.baseRegistry = process.env.CODER_BASE_REGISTRY.trim();
    source = 'env var CODER_BASE_REGISTRY';
  }
  if (process.env.CODER_BUILDER_REGISTRY != null) {
    cfg.builderRegistry = process.env.CODER_BUILDER_REGISTRY.trim();
    source = 'env var CODER_BUILDER_REGISTRY';
  }
  if (process.env.CODER_CRATE_REGISTRY != null) {
    cfg.crateRegistry = process.env.CODER_CRATE_REGISTRY.trim();
    source = 'env var CODER_CRATE_REGISTRY';
  }
  if (process.env.CODER_CRATE_TOKEN != null) {
    cfg.crateToken = process.env.CODER_CRATE_TOKEN.trim();
    source = 'env var CODER_CRATE_TOKEN';
  }

  return { cfg, source };
}

async function prompt(rl, question, defaultValue, validator) {
  const suffix = defaultValue ? ` [${defaultValue}]` : ' [留空=Docker Hub]';
  while (true) {
    const answer = await rl.question(`${question}${suffix}: `);
    const raw = (answer || '').trim();
    const value = raw === '' ? defaultValue : raw;
    const v = validator(value);
    if (v.ok) return v.value;
    log.red(`  invalid: ${v.reason}`);
  }
}

// 隐藏输入（token 等敏感信息）。回车确认；Ctrl+C/Ctrl+D 退出；
// Backspace 删除上一个字符。返回用户输入字符串（不含尾随换行）。
async function promptSecret(rl, question) {
  const stdin = rl.input;
  process.stdout.write(`${question}: `);
  return new Promise((resolve) => {
    let value = '';
    let done = false;
    const finish = (val) => {
      if (done) return;
      done = true;
      try { stdin.setRawMode(false); } catch (_) { /* not a TTY */ }
      stdin.removeListener('data', onData);
      process.stdout.write('\n');
      resolve(val);
    };
    const onData = (data) => {
      const char = data[0];
      if (char === 13 || char === 10) {
        finish(value);
      } else if (char === 3 || char === 4) {
        process.exit(0);
      } else if (char === 127 || char === 8) {
        if (value.length > 0) {
          value = value.slice(0, -1);
          process.stdout.write('\b \b');
        }
      } else if (char >= 32) {
        value += String.fromCharCode(char);
        process.stdout.write('*');
      }
    };
    try { stdin.setRawMode(true); } catch (_) { /* not a TTY, fallthrough */ }
    stdin.on('data', onData);
  });
}

async function runInteractive(existing) {
  const initialBase = existing?.baseRegistry ?? DEFAULTS.baseRegistry;
  const initialBuilder = existing?.builderRegistry ?? DEFAULTS.builderRegistry;
  const initialCrate = existing?.crateRegistry ?? DEFAULTS.crateRegistry;
  const initialToken = existing?.crateToken ?? DEFAULTS.crateToken;

  log.cyan('\n=== Docker Registry & Cargo Setup Wizard ===\n');
  log.plain('配置两类 Docker 镜像的拉取来源：\n');
  log.plain('1. 基础镜像（rabbitmq / redis / nginx / mysql / consul / grafana / prometheus 等）');
  log.plain('   默认从 Docker Hub (hub.docker.com) 拉取。若环境无法访问 Docker Hub，');
  log.plain('   输入镜像仓库地址（如 zhcoder-docker-registry.com:8000/official）作为 mirror。\n');

  const rl = readline.createInterface({ input: stdin, output: stdout });
  try {
    const baseRegistry = await prompt(
      rl,
      '  Base registry',
      initialBase || '',
      validateBaseRegistry,
    );

    log.plain('\n2. Builder 镜像（rust / java / maven / node 等编译环境）');
    log.plain('   默认从 zhcoder-docker-registry.com:8000 拉取。切换到其他私有仓库时，');
    log.plain('   输入 registry 地址（如 registry.my.com:5000）。\n');

    const builderRegistry = await prompt(
      rl,
      '  Builder registry',
      initialBuilder,
      validateBuilderRegistry,
    );

    log.plain('\n3. crates.io 镜像（Rust crate 拉取来源）');
    log.plain('   留空 = 官方 crates.io（crates.io）；');
    log.plain(`   输入镜像 URL 作为 [source.crates-io] 的 replace-with。`);
    log.plain(`   example: ${CRATE_REGISTRY_EXAMPLE}（自动补 sparse+ 与 /api/v1/crates/）`);
    log.plain('   也接受完整 URL，如 sparse+https://rsproxy.cn/index/。\n');

    const crateRegistry = await prompt(
      rl,
      '  crates.io mirror',
      initialCrate || '',
      validateCrateRegistry,
    );

    log.plain('\n4. crates.io / mirror 的 auth token');
    log.plain('   留空 = 不写 token（公开镜像如官方 crates.io / rsproxy 不需要）。');
    log.plain('   私有 kellnr 等需认证的镜像，输入 token；输入会被隐藏（显示 *）。\n');

    const crateToken = await promptSecret(
      rl,
      '  crates.io token',
    );
    // 直接回车 = 保留 existing token（若有）；输入新值则覆盖。
    const finalToken = crateToken !== '' ? crateToken : (initialToken || '');

    return {
      cfg: { baseRegistry, builderRegistry, crateRegistry, crateToken: finalToken },
      source: 'interactive prompt',
    };
  } finally {
    rl.close();
  }
}

function renderCargoConfig(normalizedRegistry) {
  const lines = [];
  if (normalizedRegistry) {
    lines.push('[source.crates-io]');
    lines.push(`replace-with = "${MIRROR_NAME}"`);
    lines.push('');
    lines.push(`[source.${MIRROR_NAME}]`);
    lines.push(`registry = "${normalizedRegistry}"`);
  } else {
    lines.push('[source.crates-io]');
    lines.push('# 官方 crates.io，无 replace-with');
  }
  lines.push('');
  lines.push('# 私有 kellnr registry - 托管 libs/* 和 sdk/rust/* 发布的 crate');
  lines.push('# cargo publish --registry kellnr  发布到此处');
  lines.push('# cargo add <crate> --registry kellnr  从此处拉取');
  lines.push('[source.kellnr]');
  lines.push(`registry = "${KELLNR_REGISTRY}"`);
  lines.push('');
  lines.push('[net]');
  lines.push('git-fetch-with-cli = true');
  lines.push('');
  return lines.join('\n');
}

function renderCargoCredentials(normalizedRegistry, token) {
  if (!token) return null;
  const name = normalizedRegistry ? MIRROR_NAME : 'crates-io';
  const lines = [];
  lines.push('# 由 scripts/setup-wizard.js 生成；已在 .gitignore，请勿 commit。');
  lines.push(`[registry.${name}]`);
  lines.push(`token = "${token}"`);
  lines.push('');
  return lines.join('\n');
}

function applyCargoConfig(cfg) {
  fs.mkdirSync(CARGO_DIR, { recursive: true });
  const normalizedRegistry = normalizeCrateRegistry(cfg.crateRegistry);

  fs.writeFileSync(CARGO_CONFIG_PATH, renderCargoConfig(normalizedRegistry));

  const credentialsContent = renderCargoCredentials(normalizedRegistry, cfg.crateToken);
  if (credentialsContent) {
    fs.writeFileSync(CARGO_CREDENTIALS_PATH, credentialsContent);
  } else if (fs.existsSync(CARGO_CREDENTIALS_PATH)) {
    fs.unlinkSync(CARGO_CREDENTIALS_PATH);
  }
}

async function main() {
  const args = parseArgs(process.argv);

  if (args.show) {
    showCurrent();
    return;
  }

  const existing = readExistingConfig();

  const isTTY = stdin.isTTY === true;
  const useInteractive = !args.nonInteractive && !args.defaults && isTTY;

  let result;
  if (useInteractive) {
    result = await runInteractive(existing);
  } else {
    result = resolveNonInteractive(args, existing);
  }

  writeConfig(result.cfg);
  applyCargoConfig(result.cfg);
  showSummary(result.cfg, result.source);
}

main().catch((err) => {
  log.red(`[wizard] crashed: ${err.message}`);
  if (err.stack) console.error(err.stack);
  process.exit(1);
});
