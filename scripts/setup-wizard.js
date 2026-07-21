#!/usr/bin/env node
/**
 * scripts/setup-wizard.js
 *
 * 交互式配置 Docker 镜像拉取来源。生成 infrastructure/.docker-registry.json，
 * 由 scripts/base-images-sync.js 读取后写入 base-images.json 的 mirror / ref 字段。
 *
 * 两类镜像：
 *   1. 基础镜像（rabbitmq / redis / nginx / mysql / consul / grafana 等）
 *      - 留空 = Docker Hub（hub.docker.com），mirror=null
 *      - 输入 registry URL = 作为 mirror，images-pull 会先尝试 `<baseRegistry>/<ref>`
 *   2. Builder 镜像（rust / java / maven / node 等编译环境）
 *      - 默认 zhcoder-docker-registry.com:8000
 *      - 输入其他 registry = 重写 ref 的 registry 前缀
 *
 * 优先级（高 -> 低）：CLI 参数 > 环境变量 > 现有配置文件 > 默认值
 * 非交互模式：stdin 不是 TTY，或传 --non-interactive / --defaults 时启用。
 *
 * 用法：
 *   node scripts/setup-wizard.js                 # 交互式（默认）
 *   node scripts/setup-wizard.js --non-interactive
 *   node scripts/setup-wizard.js --defaults      # 强制用默认值，跳过交互
 *   node scripts/setup-wizard.js --show          # 打印当前配置后退出
 *   CODER_BASE_REGISTRY=... CODER_BUILDER_REGISTRY=... node scripts/setup-wizard.js --non-interactive
 */

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const readline = require('node:readline/promises');
const { stdin, stdout } = require('node:process');

const { ROOT, log } = require('./lib/utils.js');
const { DEFAULT_BUILDER_REGISTRY } = require('./lib/Base-Images-Writer.js');

const CONFIG_PATH = path.join(ROOT, 'infrastructure', '.docker-registry.json');

const DEFAULTS = {
  baseRegistry: '',
  builderRegistry: DEFAULT_BUILDER_REGISTRY,
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
  console.log('  --defaults          force defaults (base=docker hub, builder=zhcoder-docker-registry.com:8000)');
  console.log('  --show              print current config and exit');
  console.log('');
  console.log('env vars (override config file):');
  console.log('  CODER_BASE_REGISTRY      base images registry (empty = docker hub)');
  console.log('  CODER_BUILDER_REGISTRY   builder images registry');
}

function readExistingConfig() {
  if (!fs.existsSync(CONFIG_PATH)) return null;
  try {
    const raw = JSON.parse(fs.readFileSync(CONFIG_PATH, 'utf8'));
    return {
      baseRegistry: raw.baseRegistry != null ? String(raw.baseRegistry) : '',
      builderRegistry: raw.builderRegistry != null ? String(raw.builderRegistry) : DEFAULT_BUILDER_REGISTRY,
      updatedAt: raw.updatedAt || null,
    };
  } catch (err) {
    log.yellow(`[wizard] warning: failed to parse ${CONFIG_PATH}: ${err.message}`);
    return null;
  }
}

function writeConfig(cfg) {
  const data = {
    version: 1,
    baseRegistry: cfg.baseRegistry,
    builderRegistry: cfg.builderRegistry,
    updatedAt: new Date().toISOString(),
  };
  fs.mkdirSync(path.dirname(CONFIG_PATH), { recursive: true });
  fs.writeFileSync(CONFIG_PATH, JSON.stringify(data, null, 2) + '\n');
}

function showSummary(cfg, source) {
  const baseDisplay = cfg.baseRegistry || '(docker hub)';
  log.green(`[wizard] config written to ${path.relative(ROOT, CONFIG_PATH)}`);
  log.plain(`[wizard] source:            ${source}`);
  log.plain(`[wizard] base registry:     ${baseDisplay}`);
  log.plain(`[wizard] builder registry:  ${cfg.builderRegistry}`);
  log.plain('');
  log.plain('[wizard] next: `pnpm run images:sync` will apply these to base-images.json');
}

function showCurrent() {
  const existing = readExistingConfig();
  if (!existing) {
    log.yellow(`[wizard] no config at ${path.relative(ROOT, CONFIG_PATH)}`);
    log.plain('[wizard] defaults would be:');
    log.plain(`  base registry:     (docker hub)`);
    log.plain(`  builder registry: ${DEFAULTS.builderRegistry}`);
    return;
  }
  log.green(`[wizard] current config at ${path.relative(ROOT, CONFIG_PATH)}`);
  log.plain(`  base registry:     ${existing.baseRegistry || '(docker hub)'}`);
  log.plain(`  builder registry:  ${existing.builderRegistry}`);
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

function resolveNonInteractive(args, existing) {
  // CLI --defaults > env vars > existing config > defaults
  if (args.defaults) return { cfg: { ...DEFAULTS }, source: 'defaults (forced)' };

  const cfg = { ...DEFAULTS };
  let source = 'defaults';

  if (existing) {
    cfg.baseRegistry = existing.baseRegistry;
    cfg.builderRegistry = existing.builderRegistry;
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

async function runInteractive(existing) {
  const initialBase = existing?.baseRegistry ?? DEFAULTS.baseRegistry;
  const initialBuilder = existing?.builderRegistry ?? DEFAULTS.builderRegistry;

  log.cyan('\n=== Docker Registry Setup Wizard ===\n');
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

    return {
      cfg: { baseRegistry, builderRegistry },
      source: 'interactive prompt',
    };
  } finally {
    rl.close();
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
  showSummary(result.cfg, result.source);
}

main().catch((err) => {
  log.red(`[wizard] crashed: ${err.message}`);
  if (err.stack) console.error(err.stack);
  process.exit(1);
});
