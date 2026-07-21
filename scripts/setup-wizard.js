#!/usr/bin/env node
/**
 * scripts/setup-wizard.js
 *
 * 交互式配置 Docker 镜像拉取来源 + Rust crates.io 镜像与 token
 * + Maven 仓库镜像与 token + npm 仓库镜像与 token。
 *
 * 输出文件（均 gitignored，本地配置不入 git）：
 *   - infrastructure/.docker-registry.json
 *       baseRegistry / builderRegistry，由 scripts/base-images-sync.js 读取。
 *   - .cargo/config.toml
 *       [source.crates-io] replace-with + [source.coder-mirror] / [source.kellnr] / [net]。
 *       会被覆盖式重写（保留 kellnr 私有 registry 与 git-fetch-with-cli）。
 *   - .cargo/credentials.toml
 *       [registry.<name>] token = "..."（仅当 crateToken 非空时生成）。
 *   - .mvn/settings.xml + .mvn/maven.config
 *       Maven mirror 替换 central；<servers> 同时覆盖 mirror + 发布仓库 id。
 *       认证模式：none / token（公网发布，username 固定 "token"）
 *                 / userpass（私有部署 reposilite/nexus 的 username + password）。
 *       maven.config 含 "-s.mvn/settings.xml"，mvn 启动时自动加载。
 *   - .npmrc（项目根）
 *       registry=... + //host/:_authToken=...（仅当 npmToken 非空时写 _authToken）。
 *
 * 两类 Docker 镜像：
 *   1. 基础镜像（rabbitmq / redis / nginx / mysql / consul / grafana 等）
 *      - 留空 = Docker Hub（hub.docker.com），mirror=null
 *      - 输入 registry URL = 作为 mirror，images-pull 会先尝试 `<baseRegistry>/<ref>`
 *   2. Builder 镜像（rust / java / maven / node 等编译环境）
 *      - 默认 zhcoder-docker-registry.com:8000
 *      - 输入其他 registry = 重写 ref 的 registry 前缀
 *
 * crates.io / maven / npm 镜像：
 *   - 留空 = 官方源（crates.io / Maven Central / npmjs.com）
 *   - 输入 URL = 作为私有 mirror 替换官方源
 *   - crates.io 自动补 sparse+ 前缀与 /api/v1/crates/（kellnr 默认路径）
 *   - token：留空 = 不写 credentials；非空 = 写入对应 credentials 文件。
 *   - maven 认证：none = 不写 <servers>；token = 公网 project token；
 *                 userpass = 私有部署 username + password（覆盖拉取 + 发布）。
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
 *     CODER_CRATE_REGISTRY=... CODER_CRATE_TOKEN=... \
 *     CODER_MAVEN_REGISTRY=... CODER_MAVEN_AUTH_MODE=token|userpass \
 *     CODER_MAVEN_TOKEN=... CODER_MAVEN_USERNAME=... CODER_MAVEN_PASSWORD=... \
 *     CODER_NPM_REGISTRY=... CODER_NPM_TOKEN=... \
 *     node scripts/setup-wizard.js --non-interactive
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

const MVN_DIR = path.join(ROOT, '.mvn');
const MVN_SETTINGS_PATH = path.join(MVN_DIR, 'settings.xml');
const MVN_CONFIG_PATH = path.join(MVN_DIR, 'maven.config');
const NPMRC_PATH = path.join(ROOT, '.npmrc');

// 项目共享的私有 kellnr registry（libs/* 和 sdk/rust/* 发布到此处）。
// wizard 重生成 .cargo/config.toml 时始终保留这一段。
const KELLNR_REGISTRY = 'sparse+http://192.168.2.12:16444/api/v1/crates/';
// crates.io 镜像替换后的 source 名字（[source.coder-mirror]）。
const MIRROR_NAME = 'coder-mirror';
// 交互提示用的 example URL（与 kellnr 同 host，便于团队识别）。
const CRATE_REGISTRY_EXAMPLE = 'http://192.168.2.12:16444';
const MVN_MIRROR_ID = 'coder-maven-mirror';
// sdk/java/pom.xml <distributionManagement> 的 repository id；
// <servers> 段需覆盖这些 id，mvn deploy 才能认证。
const MVN_RELEASES_ID = 'reposilite-repository-releases';
const MVN_SNAPSHOTS_ID = 'reposilite-repository-snapshots';
const MVN_REGISTRY_EXAMPLE = 'http://mvn.kudi.repos';
const NPM_REGISTRY_EXAMPLE = 'http://npm.kudi.repos';

const DEFAULTS = {
  baseRegistry: '',
  builderRegistry: DEFAULT_BUILDER_REGISTRY,
  crateRegistry: '',
  crateToken: '',
  mavenRegistry: '',
  mavenAuthMode: 'none',
  mavenToken: '',
  mavenUsername: '',
  mavenPassword: '',
  npmRegistry: '',
  npmToken: '',
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
  console.log('  --defaults          force defaults (all registries = official, no tokens)');
  console.log('  --show              print current config and exit');
  console.log('');
  console.log('env vars (override config file):');
  console.log('  CODER_BASE_REGISTRY      base images registry (empty = docker hub)');
  console.log('  CODER_BUILDER_REGISTRY   builder images registry');
  console.log('  CODER_CRATE_REGISTRY     crates.io mirror URL (empty = official crates.io)');
  console.log('  CODER_CRATE_TOKEN        crates.io / mirror auth token');
  console.log('  CODER_MAVEN_REGISTRY     maven mirror URL (empty = Maven Central)');
  console.log('  CODER_MAVEN_AUTH_MODE    maven auth: none | token | userpass');
  console.log('  CODER_MAVEN_TOKEN        maven token (auth=token; username fixed to "token")');
  console.log('  CODER_MAVEN_USERNAME     maven username (auth=userpass; private reposilite/nexus)');
  console.log('  CODER_MAVEN_PASSWORD     maven password (auth=userpass)');
  console.log('  CODER_NPM_REGISTRY       npm mirror URL (empty = npmjs.com)');
  console.log('  CODER_NPM_TOKEN          npm mirror auth token (_authToken)');
}

function readExistingConfig() {
  if (!fs.existsSync(CONFIG_PATH)) return null;
  try {
    const raw = JSON.parse(fs.readFileSync(CONFIG_PATH, 'utf8'));
    const authMode = raw.mavenAuthMode != null ? String(raw.mavenAuthMode) : 'none';
    // 兼容 v3 旧字段 mavenToken：若旧 token 非空且 authMode 未显式设置，升级为 token 模式。
    const legacyToken = raw.mavenToken != null ? String(raw.mavenToken) : '';
    const migratedAuthMode = authMode === 'none' && legacyToken ? 'token' : authMode;
    return {
      baseRegistry: raw.baseRegistry != null ? String(raw.baseRegistry) : '',
      builderRegistry: raw.builderRegistry != null ? String(raw.builderRegistry) : DEFAULT_BUILDER_REGISTRY,
      crateRegistry: raw.crateRegistry != null ? String(raw.crateRegistry) : '',
      crateToken: raw.crateToken != null ? String(raw.crateToken) : '',
      mavenRegistry: raw.mavenRegistry != null ? String(raw.mavenRegistry) : '',
      mavenAuthMode: migratedAuthMode,
      mavenToken: raw.mavenToken != null ? String(raw.mavenToken) : '',
      mavenUsername: raw.mavenUsername != null ? String(raw.mavenUsername) : '',
      mavenPassword: raw.mavenPassword != null ? String(raw.mavenPassword) : '',
      npmRegistry: raw.npmRegistry != null ? String(raw.npmRegistry) : '',
      npmToken: raw.npmToken != null ? String(raw.npmToken) : '',
      updatedAt: raw.updatedAt || null,
    };
  } catch (err) {
    log.yellow(`[wizard] warning: failed to parse ${CONFIG_PATH}: ${err.message}`);
    return null;
  }
}

function writeConfig(cfg) {
  const data = {
    version: 4,
    baseRegistry: cfg.baseRegistry,
    builderRegistry: cfg.builderRegistry,
    crateRegistry: cfg.crateRegistry,
    crateToken: cfg.crateToken,
    mavenRegistry: cfg.mavenRegistry,
    mavenAuthMode: cfg.mavenAuthMode,
    mavenToken: cfg.mavenToken,
    mavenUsername: cfg.mavenUsername,
    mavenPassword: cfg.mavenPassword,
    npmRegistry: cfg.npmRegistry,
    npmToken: cfg.npmToken,
    updatedAt: new Date().toISOString(),
  };
  fs.mkdirSync(path.dirname(CONFIG_PATH), { recursive: true });
  fs.writeFileSync(CONFIG_PATH, JSON.stringify(data, null, 2) + '\n');
}

function mavenAuthSummary(cfg) {
  if (cfg.mavenAuthMode === 'token') return `token: ${cfg.mavenToken ? '(set)' : '(missing!)'}`;
  if (cfg.mavenAuthMode === 'userpass') {
    return `userpass: ${cfg.mavenUsername || '(missing!)'} / ${cfg.mavenPassword ? '(set)' : '(missing!)'}`;
  }
  return 'auth: (none)';
}

function showSummary(cfg, source) {
  const baseDisplay = cfg.baseRegistry || '(docker hub)';
  const crateDisplay = cfg.crateRegistry || '(official crates.io)';
  const crateTokenDisplay = cfg.crateToken ? '(set)' : '(none)';
  const mvnDisplay = cfg.mavenRegistry || '(Maven Central)';
  const npmDisplay = cfg.npmRegistry || '(npmjs.com)';
  const npmTokenDisplay = cfg.npmToken ? '(set)' : '(none)';
  log.green(`[wizard] config written to ${path.relative(ROOT, CONFIG_PATH)}`);
  log.plain(`[wizard] source:            ${source}`);
  log.plain(`[wizard] base registry:     ${baseDisplay}`);
  log.plain(`[wizard] builder registry:  ${cfg.builderRegistry}`);
  log.plain(`[wizard] crates.io mirror:  ${crateDisplay}  token: ${crateTokenDisplay}`);
  log.plain(`[wizard] maven mirror:      ${mvnDisplay}  ${mavenAuthSummary(cfg)}`);
  log.plain(`[wizard] npm mirror:        ${npmDisplay}  token: ${npmTokenDisplay}`);
  log.plain('');
  log.plain('[wizard] next: `pnpm run images:sync` will apply these to base-images.json');
  log.plain('[wizard]       .cargo/config.toml regenerated (kellnr + [net] preserved)');
  if (cfg.crateToken) {
    log.yellow('[wizard] crate token -> .cargo/credentials.toml (gitignored).');
    log.plain(`[wizard]   apply: export CARGO_CREDENTIALS_PATH="${path.relative(ROOT, CARGO_CREDENTIALS_PATH)}"`);
  }
  if (cfg.mavenRegistry || cfg.mavenAuthMode !== 'none') {
    log.yellow('[wizard] maven -> .mvn/settings.xml + .mvn/maven.config (gitignored).');
    log.plain('[wizard]   <servers> covers mirror + releases + snapshots (mvn deploy).');
    log.plain('[wizard]   mvn auto-loads via .mvn/maven.config (-s.mvn/settings.xml).');
  }
  if (cfg.npmRegistry) {
    log.yellow('[wizard] npm mirror -> .npmrc (gitignored, project root).');
    log.plain('[wizard]   npm/pnpm auto-loads project-level .npmrc.');
  }
}

function showCurrent() {
  const existing = readExistingConfig();
  if (!existing) {
    log.yellow(`[wizard] no config at ${path.relative(ROOT, CONFIG_PATH)}`);
    log.plain('[wizard] defaults would be:');
    log.plain(`  base registry:     (docker hub)`);
    log.plain(`  builder registry: ${DEFAULTS.builderRegistry}`);
    log.plain(`  crates.io mirror: (official)   token: (none)`);
    log.plain(`  maven mirror:     (Maven Central) auth: (none)`);
    log.plain(`  npm mirror:       (npmjs.com)   token: (none)`);
    return;
  }
  log.green(`[wizard] current config at ${path.relative(ROOT, CONFIG_PATH)}`);
  log.plain(`  base registry:     ${existing.baseRegistry || '(docker hub)'}`);
  log.plain(`  builder registry:  ${existing.builderRegistry}`);
  log.plain(`  crates.io mirror:  ${existing.crateRegistry || '(official crates.io)'}   token: ${existing.crateToken ? '(set)' : '(none)'}`);
  log.plain(`  maven mirror:      ${existing.mavenRegistry || '(Maven Central)'}   ${mavenAuthSummary(existing)}`);
  log.plain(`  npm mirror:        ${existing.npmRegistry || '(npmjs.com)'}   token: ${existing.npmToken ? '(set)' : '(none)'}`);
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

// maven / npm registry URL 校验：留空 = 官方源；非空 = 不能含空白。
// 不做 URL 严格校验（用户可能填 http://host:port 或 https://host/path）。
function validateMavenRegistry(value) {
  const v = (value || '').trim();
  if (v && /\s/.test(v)) return { ok: false, reason: 'must not contain whitespace' };
  return { ok: true, value: v };
}

function validateNpmRegistry(value) {
  const v = (value || '').trim();
  if (v && /\s/.test(v)) return { ok: false, reason: 'must not contain whitespace' };
  return { ok: true, value: v };
}

function validateCrateToken(value) {
  const v = (value || '').trim();
  if (v && /\s/.test(v)) return { ok: false, reason: 'must not contain whitespace' };
  return { ok: true, value: v };
}

// maven 认证模式：none / token / userpass。
// token = 公网发布（如 Maven Central project token，username 固定 "token"）；
// userpass = 私有部署 reposilite / nexus 的 username + password。
function validateMavenAuthMode(value) {
  const v = (value || '').trim().toLowerCase();
  if (v === '' || v === 'none') return { ok: true, value: 'none' };
  if (v === 'token' || v === 'userpass') return { ok: true, value: v };
  return { ok: false, reason: 'must be one of: none | token | userpass' };
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
    cfg.mavenRegistry = existing.mavenRegistry;
    cfg.mavenAuthMode = existing.mavenAuthMode;
    cfg.mavenToken = existing.mavenToken;
    cfg.mavenUsername = existing.mavenUsername;
    cfg.mavenPassword = existing.mavenPassword;
    cfg.npmRegistry = existing.npmRegistry;
    cfg.npmToken = existing.npmToken;
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
  if (process.env.CODER_MAVEN_REGISTRY != null) {
    cfg.mavenRegistry = process.env.CODER_MAVEN_REGISTRY.trim();
    source = 'env var CODER_MAVEN_REGISTRY';
  }
  if (process.env.CODER_MAVEN_AUTH_MODE != null) {
    const m = validateMavenAuthMode(process.env.CODER_MAVEN_AUTH_MODE);
    if (!m.ok) {
      log.red(`[wizard] invalid CODER_MAVEN_AUTH_MODE: ${m.reason}`);
      process.exit(1);
    }
    cfg.mavenAuthMode = m.value;
    source = 'env var CODER_MAVEN_AUTH_MODE';
  }
  if (process.env.CODER_MAVEN_TOKEN != null) {
    cfg.mavenToken = process.env.CODER_MAVEN_TOKEN.trim();
    if (cfg.mavenAuthMode === 'none') cfg.mavenAuthMode = 'token';
    source = 'env var CODER_MAVEN_TOKEN';
  }
  if (process.env.CODER_MAVEN_USERNAME != null) {
    cfg.mavenUsername = process.env.CODER_MAVEN_USERNAME.trim();
    if (cfg.mavenAuthMode === 'none') cfg.mavenAuthMode = 'userpass';
    source = 'env var CODER_MAVEN_USERNAME';
  }
  if (process.env.CODER_MAVEN_PASSWORD != null) {
    cfg.mavenPassword = process.env.CODER_MAVEN_PASSWORD.trim();
    if (cfg.mavenAuthMode === 'none') cfg.mavenAuthMode = 'userpass';
    source = 'env var CODER_MAVEN_PASSWORD';
  }
  if (process.env.CODER_NPM_REGISTRY != null) {
    cfg.npmRegistry = process.env.CODER_NPM_REGISTRY.trim();
    source = 'env var CODER_NPM_REGISTRY';
  }
  if (process.env.CODER_NPM_TOKEN != null) {
    cfg.npmToken = process.env.CODER_NPM_TOKEN.trim();
    source = 'env var CODER_NPM_TOKEN';
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

// 明文输入 token / password 等敏感值。
// 不在提示中回显 existing 值（避免明文泄露），只提示"留空=保留现有"。
// 直接回车 = 保留 existingValue；输入新值则覆盖。
async function promptVisible(rl, question, existingValue, validator) {
  const suffix = existingValue ? ' [留空=保留现有]' : ' [留空=跳过]';
  while (true) {
    const answer = await rl.question(`${question}${suffix}: `);
    const raw = (answer || '').trim();
    const value = raw === '' ? (existingValue || '') : raw;
    const v = validator(value);
    if (v.ok) return v.value;
    log.red(`  invalid: ${v.reason}`);
  }
}

async function runInteractive(existing) {
  const initialBase = existing?.baseRegistry ?? DEFAULTS.baseRegistry;
  const initialBuilder = existing?.builderRegistry ?? DEFAULTS.builderRegistry;
  const initialCrate = existing?.crateRegistry ?? DEFAULTS.crateRegistry;
  const initialCrateToken = existing?.crateToken ?? DEFAULTS.crateToken;
  const initialMaven = existing?.mavenRegistry ?? DEFAULTS.mavenRegistry;
  const initialMavenAuthMode = existing?.mavenAuthMode ?? DEFAULTS.mavenAuthMode;
  const initialMavenToken = existing?.mavenToken ?? DEFAULTS.mavenToken;
  const initialMavenUsername = existing?.mavenUsername ?? DEFAULTS.mavenUsername;
  const initialMavenPassword = existing?.mavenPassword ?? DEFAULTS.mavenPassword;
  const initialNpm = existing?.npmRegistry ?? DEFAULTS.npmRegistry;
  const initialNpmToken = existing?.npmToken ?? DEFAULTS.npmToken;

  log.cyan('\n=== Docker Registry & Cargo / Maven / Npm Setup Wizard ===\n');
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
    log.plain('   私有 kellnr 等需认证的镜像，输入 token（明文输入）。\n');

    const crateToken = await promptVisible(rl, '  crates.io token', initialCrateToken, validateCrateToken);

    log.plain('\n5. Maven 镜像（Java 依赖拉取来源）');
    log.plain('   留空 = 官方 Maven Central（repo.maven.apache.org）；');
    log.plain(`   输入镜像 URL 作为 <mirror> 替换 central。example: ${MVN_REGISTRY_EXAMPLE}`);
    log.plain('   生成 .mvn/settings.xml + .mvn/maven.config（mvn 自动加载）。\n');

    const mavenRegistry = await prompt(
      rl,
      '  maven mirror',
      initialMaven || '',
      validateMavenRegistry,
    );

    log.plain('\n6. Maven 认证方式（拉取 mirror + mvn deploy 发布都用此认证）');
    log.plain('   none     = 不写 <servers>（公开 mirror 如 Maven Central 无需认证）');
    log.plain('   token    = 公网发布（如 Maven Central project token；username 固定 "token"）');
    log.plain('   userpass = 私有部署（reposilite / nexus 的 username + password）\n');

    const mavenAuthModeInput = await prompt(
      rl,
      '  maven auth mode [none/token/userpass]',
      initialMavenAuthMode,
      validateMavenAuthMode,
    );
    const mavenAuthMode = mavenAuthModeInput;

    let mavenToken = initialMavenToken;
    let mavenUsername = initialMavenUsername;
    let mavenPassword = initialMavenPassword;

    if (mavenAuthMode === 'token') {
      log.plain('\n   输入 token（公网 project token，明文输入）。');
      log.plain('   直接回车 = 保留 existing token（若有）。\n');
      mavenToken = await promptVisible(rl, '  maven token', initialMavenToken, validateCrateToken);
      mavenUsername = '';
      mavenPassword = '';
    } else if (mavenAuthMode === 'userpass') {
      log.plain('\n   输入 username + password（私有 reposilite / nexus，明文输入）。');
      log.plain('   直接回车 = 保留 existing 值（若有）。\n');
      mavenUsername = await promptVisible(rl, '  maven username', initialMavenUsername, validateCrateToken);
      mavenPassword = await promptVisible(rl, '  maven password', initialMavenPassword, validateCrateToken);
      mavenToken = '';
    } else {
      mavenToken = '';
      mavenUsername = '';
      mavenPassword = '';
    }

    log.plain('\n7. npm 镜像（Node 依赖拉取来源）');
    log.plain('   留空 = 官方 npmjs.com（registry.npmjs.org）；');
    log.plain(`   输入镜像 URL 作为 registry。example: ${NPM_REGISTRY_EXAMPLE}`);
    log.plain('   生成项目根 .npmrc（npm/pnpm 自动加载）。\n');

    const npmRegistry = await prompt(
      rl,
      '  npm mirror',
      initialNpm || '',
      validateNpmRegistry,
    );

    log.plain('\n8. npm mirror 的 auth token');
    log.plain('   留空 = 不写 _authToken；');
    log.plain('   私有 verdaccio/nexus 等需认证的镜像，输入 token（写入 //host/:_authToken，明文输入）。\n');

    const npmToken = await promptVisible(rl, '  npm token', initialNpmToken, validateCrateToken);

    return {
      cfg: {
        baseRegistry,
        builderRegistry,
        crateRegistry,
        crateToken,
        mavenRegistry,
        mavenAuthMode,
        mavenToken,
        mavenUsername,
        mavenPassword,
        npmRegistry,
        npmToken,
      },
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

// 从 registry URL 提取 "//host[:port]/" 形式，用于 npm _authToken 的 key。
// npm 规范：_authToken 的 key 是不含协议的 host:port 段，前缀 //，后缀 /。
// 例：http://npm.kudi.repos -> //npm.kudi.repos/
//      https://registry.npmjs.org/ -> //registry.npmjs.org/
function npmAuthKey(registryUrl) {
  try {
    const u = new URL(registryUrl);
    return `//${u.host}/`;
  } catch (_) {
    return null;
  }
}

// 生成 <server> 段：根据 authMode 决定 username/password。
// token 模式：username 固定 "token"，password = token（公网发布）。
// userpass 模式：username + password 用户输入（私有部署）。
// 覆盖 3 个 id：mirror（拉取）+ releases + snapshots（mvn deploy 发布）。
function renderMavenServers(authMode, token, username, password) {
  if (authMode !== 'token' && authMode !== 'userpass') return [];
  const user = authMode === 'token' ? 'token' : username;
  const pass = authMode === 'token' ? token : password;
  if (!pass) return [];
  const ids = [MVN_MIRROR_ID, MVN_RELEASES_ID, MVN_SNAPSHOTS_ID];
  const lines = ['  <servers>'];
  for (const id of ids) {
    lines.push('    <server>');
    lines.push(`      <id>${id}</id>`);
    lines.push(`      <username>${user}</username>`);
    lines.push(`      <password>${pass}</password>`);
    lines.push('    </server>');
  }
  lines.push('  </servers>');
  return lines;
}

function renderMavenSettings(registry, authMode, token, username, password) {
  const lines = [];
  lines.push('<?xml version="1.0" encoding="UTF-8"?>');
  lines.push('<!-- 由 scripts/setup-wizard.js 生成；已在 .gitignore，请勿 commit。 -->');
  lines.push('<settings xmlns="http://maven.apache.org/SETTINGS/1.0.0"');
  lines.push('          xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"');
  lines.push('          xsi:schemaLocation="http://maven.apache.org/SETTINGS/1.0.0 https://maven.apache.org/xsd/settings-1.0.0.xsd">');
  if (registry) {
    lines.push('  <mirrors>');
    lines.push('    <mirror>');
    lines.push(`      <id>${MVN_MIRROR_ID}</id>`);
    lines.push('      <mirrorOf>central</mirrorOf>');
    lines.push(`      <url>${registry}</url>`);
    lines.push('    </mirror>');
    lines.push('  </mirrors>');
  }
  lines.push(...renderMavenServers(authMode, token, username, password));
  lines.push('</settings>');
  lines.push('');
  return lines.join('\n');
}

function applyMavenConfig(cfg) {
  const hasMirror = !!cfg.mavenRegistry;
  const hasAuth = cfg.mavenAuthMode === 'token' || cfg.mavenAuthMode === 'userpass';
  if (!hasMirror && !hasAuth) {
    if (fs.existsSync(MVN_SETTINGS_PATH)) fs.unlinkSync(MVN_SETTINGS_PATH);
    if (fs.existsSync(MVN_CONFIG_PATH)) fs.unlinkSync(MVN_CONFIG_PATH);
    return;
  }
  fs.mkdirSync(MVN_DIR, { recursive: true });
  fs.writeFileSync(
    MVN_SETTINGS_PATH,
    renderMavenSettings(cfg.mavenRegistry, cfg.mavenAuthMode, cfg.mavenToken, cfg.mavenUsername, cfg.mavenPassword),
  );
  fs.writeFileSync(MVN_CONFIG_PATH, '-s.mvn/settings.xml\n');
}

function renderNpmrc(registry, token) {
  const lines = [];
  lines.push('# 由 scripts/setup-wizard.js 生成；已在 .gitignore，请勿 commit。');
  lines.push(`registry=${registry}`);
  if (token) {
    const authKey = npmAuthKey(registry);
    if (authKey) {
      lines.push(`${authKey}:_authToken=${token}`);
    }
  }
  lines.push('');
  return lines.join('\n');
}

function applyNpmConfig(cfg) {
  if (!cfg.npmRegistry) {
    if (fs.existsSync(NPMRC_PATH)) fs.unlinkSync(NPMRC_PATH);
    return;
  }
  fs.writeFileSync(NPMRC_PATH, renderNpmrc(cfg.npmRegistry, cfg.npmToken));
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
  applyMavenConfig(result.cfg);
  applyNpmConfig(result.cfg);
  showSummary(result.cfg, result.source);
}

main().catch((err) => {
  log.red(`[wizard] crashed: ${err.message}`);
  if (err.stack) console.error(err.stack);
  process.exit(1);
});
