#!/usr/bin/env node
/**
 * scripts/setup-wizard.js
 *
 * 交互式配置 Docker 镜像拉取来源 + Rust crates.io 镜像与 token
 * + Maven 仓库镜像与 token + npm 仓库镜像与 token。
 * 可选同步到 .gitlab-ci.yml（CODER_* 变量写入 variables: 段）。
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
 *     CODER_SYNC_GITLAB_CI=yes \
 *     node scripts/setup-wizard.js --non-interactive
 */

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const prompts = require('prompts');

const { ROOT } = require('./lib/utils.js');
const { chalk } = require('./lib/ui.js');
const { DEFAULT_BUILDER_REGISTRY } = require('./lib/Base-Images-Writer.js');

const CONFIG_PATH = path.join(ROOT, 'infrastructure', '.docker-registry.json');
const CARGO_DIR = path.join(ROOT, '.cargo');
const CARGO_CONFIG_PATH = path.join(CARGO_DIR, 'config.toml');
const CARGO_CREDENTIALS_PATH = path.join(CARGO_DIR, 'credentials.toml');

const MVN_DIR = path.join(ROOT, '.mvn');
const MVN_SETTINGS_PATH = path.join(MVN_DIR, 'settings.xml');
const MVN_CONFIG_PATH = path.join(MVN_DIR, 'maven.config');
const NPMRC_PATH = path.join(ROOT, '.npmrc');
const GITLAB_CI_PATH = path.join(ROOT, '.gitlab-ci.yml');

// 项目共享的私有 kellnr registry（libs/* 和 sdk/rust/* 发布到此处）。
// wizard 重生成 .cargo/config.toml 时始终保留这一段。
const KELLNR_REGISTRY = 'sparse+http://192.168.2.12:16444/api/v1/crates/';
// crates.io 镜像替换后的 source 名字（[source.coder-mirror]）。
const MIRROR_NAME = 'coder-mirror';
// 交互提示用的 example URL：必须是 crates.io 公网镜像。私有 kellnr（192.168.2.12:16444）
// 不镜像公网 crate，填进去会让 crates-io 找不到 anyhow/serde 等——不要用作 example。
const CRATE_REGISTRY_EXAMPLE = 'https://rsproxy.cn/index/';
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
  // 私有 crate registry：托管 libs/* 和 sdk/rust/* 的发布 + 拉取，与 crateRegistry 语义独立。
  privateCrateRegistry: '',
  privateCrateRegistryToken: '',
  mavenRegistry: '',
  mavenAuthMode: 'none',
  mavenToken: '',
  mavenUsername: '',
  mavenPassword: '',
  npmRegistry: '',
  npmToken: '',
};

// wizard 字段 -> .gitlab-ci.yml variables 段的 CI 变量映射。
// sensitive=true 的字段写 "$CI_VAR" 引用（需在 GitLab Settings -> CI/CD -> Variables 创建），
// sensitive=false 的字段直接写明文值。
const CI_FIELD_MAP = [
  { field: 'baseRegistry',              var: 'CODER_BASE_REGISTRY',              sensitive: false },
  { field: 'builderRegistry',           var: 'CODER_BUILDER_REGISTRY',           sensitive: false },
  { field: 'crateRegistry',             var: 'CODER_CRATE_REGISTRY',             sensitive: false },
  { field: 'crateToken',                var: 'CODER_CRATE_TOKEN',                sensitive: true, ciVar: 'CRATE_TOKEN' },
  { field: 'privateCrateRegistry',      var: 'CODER_PRIVATE_CRATE_REGISTRY',     sensitive: false },
  { field: 'privateCrateRegistryToken', var: 'CODER_PRIVATE_CRATE_REGISTRY_TOKEN', sensitive: true, ciVar: 'PRIVATE_CRATE_REGISTRY_TOKEN' },
  { field: 'mavenRegistry',             var: 'CODER_MAVEN_REGISTRY',             sensitive: false },
  { field: 'mavenAuthMode',             var: 'CODER_MAVEN_AUTH_MODE',            sensitive: false },
  { field: 'mavenToken',                var: 'CODER_MAVEN_TOKEN',                sensitive: true, ciVar: 'MAVEN_TOKEN' },
  { field: 'mavenUsername',             var: 'CODER_MAVEN_USERNAME',             sensitive: true, ciVar: 'MAVEN_USERNAME' },
  { field: 'mavenPassword',             var: 'CODER_MAVEN_PASSWORD',             sensitive: true, ciVar: 'MAVEN_PASSWORD' },
  { field: 'npmRegistry',               var: 'CODER_NPM_REGISTRY',               sensitive: false },
  { field: 'npmToken',                  var: 'CODER_NPM_TOKEN',                  sensitive: true, ciVar: 'NPM_TOKEN' },
];

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
      console.error(chalk.red(`unknown argument: ${a}`));
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
  console.log('  CODER_CRATE_REGISTRY         crates.io mirror URL (empty = official crates.io)');
  console.log('  CODER_CRATE_TOKEN            crates.io / mirror auth token');
  console.log('  CODER_PRIVATE_CRATE_REGISTRY private crate registry (libs/* sdk/rust/*); empty = none');
  console.log('  CODER_PRIVATE_CRATE_REGISTRY_TOKEN private crate registry auth token');
  console.log('  CODER_MAVEN_REGISTRY     maven mirror URL (empty = Maven Central)');
  console.log('  CODER_MAVEN_AUTH_MODE    maven auth: none | token | userpass');
  console.log('  CODER_MAVEN_TOKEN        maven token (auth=token; username fixed to "token")');
  console.log('  CODER_MAVEN_USERNAME     maven username (auth=userpass; private reposilite/nexus)');
  console.log('  CODER_MAVEN_PASSWORD     maven password (auth=userpass)');
  console.log('  CODER_NPM_REGISTRY       npm mirror URL (empty = npmjs.com)');
  console.log('  CODER_NPM_TOKEN          npm mirror auth token (_authToken)');
  console.log('  CODER_SYNC_GITLAB_CI     sync CODER_* vars to .gitlab-ci.yml (yes/1/true)');
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
      privateCrateRegistry: raw.privateCrateRegistry != null ? String(raw.privateCrateRegistry) : '',
      privateCrateRegistryToken: raw.privateCrateRegistryToken != null ? String(raw.privateCrateRegistryToken) : '',
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
    console.log(chalk.yellow(`[wizard] warning: failed to parse ${CONFIG_PATH}: ${err.message}`));
    return null;
  }
}

function writeConfig(cfg) {
  const data = {
    version: 5,
    baseRegistry: cfg.baseRegistry,
    builderRegistry: cfg.builderRegistry,
    crateRegistry: cfg.crateRegistry,
    crateToken: cfg.crateToken,
    privateCrateRegistry: cfg.privateCrateRegistry,
    privateCrateRegistryToken: cfg.privateCrateRegistryToken,
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

function showSummary(cfg, source, ciSynced) {
  const baseDisplay = cfg.baseRegistry || '(docker hub)';
  const crateDisplay = cfg.crateRegistry || '(official crates.io)';
  const crateTokenDisplay = cfg.crateToken ? '(set)' : '(none)';
  const privateDisplay = cfg.privateCrateRegistry || '(none)';
  const privateTokenDisplay = cfg.privateCrateRegistryToken ? '(set)' : '(none)';
  const mvnDisplay = cfg.mavenRegistry || '(Maven Central)';
  const npmDisplay = cfg.npmRegistry || '(npmjs.com)';
  const npmTokenDisplay = cfg.npmToken ? '(set)' : '(none)';
  console.log(chalk.green(`[wizard] config written to ${path.relative(ROOT, CONFIG_PATH)}`));
  console.log(`[wizard] source:                  ${source}`);
  console.log(`[wizard] base registry:           ${baseDisplay}`);
  console.log(`[wizard] builder registry:        ${cfg.builderRegistry}`);
  console.log(`[wizard] crates.io mirror:        ${crateDisplay}  token: ${crateTokenDisplay}`);
  console.log(`[wizard] private crate registry:  ${privateDisplay}  token: ${privateTokenDisplay}`);
  console.log(`[wizard] maven mirror:            ${mvnDisplay}  ${mavenAuthSummary(cfg)}`);
  console.log(`[wizard] npm mirror:              ${npmDisplay}  token: ${npmTokenDisplay}`);
  console.log('');
  console.log('[wizard] next: `pnpm run images:sync` will apply these to base-images.json');
  console.log('[wizard]       .cargo/config.toml regenerated (kellnr + [net] preserved)');
  if (cfg.crateToken || cfg.privateCrateRegistryToken) {
    console.log(chalk.yellow('[wizard] crate token(s) -> .cargo/credentials.toml (gitignored).'));
    console.log(`[wizard]   apply: export CARGO_CREDENTIALS_PATH="${path.relative(ROOT, CARGO_CREDENTIALS_PATH)}"`);
  }
  if (cfg.mavenRegistry || cfg.mavenAuthMode !== 'none') {
    console.log(chalk.yellow('[wizard] maven -> .mvn/settings.xml + .mvn/maven.config (gitignored).'));
    console.log('[wizard]   <servers> covers mirror + releases + snapshots (mvn deploy).');
    console.log('[wizard]   mvn auto-loads via .mvn/maven.config (-s.mvn/settings.xml).');
  }
  if (cfg.npmRegistry) {
    console.log(chalk.yellow('[wizard] npm mirror -> .npmrc (gitignored, project root).'));
    console.log('[wizard]   npm/pnpm auto-loads project-level .npmrc.');
  }
  if (ciSynced) {
    console.log('');
    console.log(chalk.green(`[wizard] .gitlab-ci.yml synced: CODER_* variables updated in variables: section.`));
    const ciVars = requiredCIVariables(cfg);
    if (ciVars.length) {
      console.log(chalk.yellow('[wizard] 敏感值引用了 GitLab CI Variables，请在 Settings -> CI/CD -> Variables 创建：'));
      for (const v of ciVars) {
        console.log(`[wizard]   - ${v}`);
      }
    }
  }
}

function showCurrent() {
  const existing = readExistingConfig();
  if (!existing) {
    console.log(chalk.yellow(`[wizard] no config at ${path.relative(ROOT, CONFIG_PATH)}`));
    console.log('[wizard] defaults would be:');
    console.log(`  base registry:     (docker hub)`);
    console.log(`  builder registry: ${DEFAULTS.builderRegistry}`);
    console.log(`  crates.io mirror: (official)   token: (none)`);
    console.log(`  maven mirror:     (Maven Central) auth: (none)`);
    console.log(`  npm mirror:       (npmjs.com)   token: (none)`);
    return;
  }
  console.log(chalk.green(`[wizard] current config at ${path.relative(ROOT, CONFIG_PATH)}`));
  console.log(`  base registry:     ${existing.baseRegistry || '(docker hub)'}`);
  console.log(`  builder registry:  ${existing.builderRegistry}`);
  console.log(`  crates.io mirror:  ${existing.crateRegistry || '(official crates.io)'}   token: ${existing.crateToken ? '(set)' : '(none)'}`);
  console.log(`  maven mirror:      ${existing.mavenRegistry || '(Maven Central)'}   ${mavenAuthSummary(existing)}`);
  console.log(`  npm mirror:        ${existing.npmRegistry || '(npmjs.com)'}   token: ${existing.npmToken ? '(set)' : '(none)'}`);
  console.log(`  updatedAt:         ${existing.updatedAt || '(unknown)'}`);
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

function validateYesNo(value) {
  const v = (value || '').trim().toLowerCase();
  if (v === 'yes' || v === 'y' || v === 'no' || v === 'n') return { ok: true, value: v };
  return { ok: false, reason: 'must be yes/y or no/n' };
}

function resolveNonInteractive(args, existing) {
  // CLI --defaults > env vars > existing config > defaults
  if (args.defaults) return { cfg: { ...DEFAULTS }, syncGitlabCI: false, source: 'defaults (forced)' };

  const cfg = { ...DEFAULTS };
  let source = 'defaults';

  if (existing) {
    cfg.baseRegistry = existing.baseRegistry;
    cfg.builderRegistry = existing.builderRegistry;
    cfg.crateRegistry = existing.crateRegistry;
    cfg.crateToken = existing.crateToken;
    cfg.privateCrateRegistry = existing.privateCrateRegistry;
    cfg.privateCrateRegistryToken = existing.privateCrateRegistryToken;
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
  if (process.env.CODER_PRIVATE_CRATE_REGISTRY != null) {
    cfg.privateCrateRegistry = process.env.CODER_PRIVATE_CRATE_REGISTRY.trim();
    source = 'env var CODER_PRIVATE_CRATE_REGISTRY';
  }
  if (process.env.CODER_PRIVATE_CRATE_REGISTRY_TOKEN != null) {
    cfg.privateCrateRegistryToken = process.env.CODER_PRIVATE_CRATE_REGISTRY_TOKEN.trim();
    source = 'env var CODER_PRIVATE_CRATE_REGISTRY_TOKEN';
  }
  if (process.env.CODER_MAVEN_REGISTRY != null) {
    cfg.mavenRegistry = process.env.CODER_MAVEN_REGISTRY.trim();
    source = 'env var CODER_MAVEN_REGISTRY';
  }
  if (process.env.CODER_MAVEN_AUTH_MODE != null) {
    const m = validateMavenAuthMode(process.env.CODER_MAVEN_AUTH_MODE);
    if (!m.ok) {
      console.error(chalk.red(`[wizard] invalid CODER_MAVEN_AUTH_MODE: ${m.reason}`));
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

  const syncGitlabCI = process.env.CODER_SYNC_GITLAB_CI === 'yes'
    || process.env.CODER_SYNC_GITLAB_CI === '1'
    || process.env.CODER_SYNC_GITLAB_CI === 'true';

  return { cfg, syncGitlabCI, source };
}

function onCancel() {
  console.error(chalk.yellow('\n✗ 操作被取消'));
  process.exit(130);
}

async function prompt(_rl, question, defaultValue, validator) {
  const suffix = defaultValue ? ` [${defaultValue}]` : ' [留空=Docker Hub]';
  while (true) {
    const res = await prompts({
      type: 'text',
      name: 'v',
      message: `${question}${suffix}`,
    }, { onCancel });
    const raw = (res.v || '').trim();
    const value = raw === '' ? defaultValue : raw;
    const v = validator(value);
    if (v.ok) return v.value;
    console.error(chalk.red(`  invalid: ${v.reason}`));
  }
}

// 明文输入 token / password 等敏感值。
// 不在提示中回显 existing 值（避免明文泄露），只提示"留空=保留现有"。
// 直接回车 = 保留 existingValue；输入新值则覆盖。
async function promptVisible(_rl, question, existingValue, validator) {
  const suffix = existingValue ? ' [留空=保留现有]' : ' [留空=跳过]';
  while (true) {
    const res = await prompts({
      type: 'text',
      name: 'v',
      message: `${question}${suffix}`,
    }, { onCancel });
    const raw = (res.v || '').trim();
    const value = raw === '' ? (existingValue || '') : raw;
    const v = validator(value);
    if (v.ok) return v.value;
    console.error(chalk.red(`  invalid: ${v.reason}`));
  }
}

async function runInteractive(existing) {
  const initialBase = existing?.baseRegistry ?? DEFAULTS.baseRegistry;
  const initialBuilder = existing?.builderRegistry ?? DEFAULTS.builderRegistry;
  const initialCrate = existing?.crateRegistry ?? DEFAULTS.crateRegistry;
  const initialCrateToken = existing?.crateToken ?? DEFAULTS.crateToken;
  const initialPrivateCrate = existing?.privateCrateRegistry ?? DEFAULTS.privateCrateRegistry;
  const initialPrivateCrateToken = existing?.privateCrateRegistryToken ?? DEFAULTS.privateCrateRegistryToken;
  const initialMaven = existing?.mavenRegistry ?? DEFAULTS.mavenRegistry;
  const initialMavenAuthMode = existing?.mavenAuthMode ?? DEFAULTS.mavenAuthMode;
  const initialMavenToken = existing?.mavenToken ?? DEFAULTS.mavenToken;
  const initialMavenUsername = existing?.mavenUsername ?? DEFAULTS.mavenUsername;
  const initialMavenPassword = existing?.mavenPassword ?? DEFAULTS.mavenPassword;
  const initialNpm = existing?.npmRegistry ?? DEFAULTS.npmRegistry;
  const initialNpmToken = existing?.npmToken ?? DEFAULTS.npmToken;

  console.log(chalk.cyan('\n=== Docker Registry & Cargo / Maven / Npm Setup Wizard ===\n'));
  console.log('配置两类 Docker 镜像的拉取来源：\n');
  console.log('1. 基础镜像（rabbitmq / redis / nginx / mysql / consul / grafana / prometheus 等）');
  console.log('   默认从 Docker Hub (hub.docker.com) 拉取。若环境无法访问 Docker Hub，');
  console.log('   输入镜像仓库地址（如 zhcoder-docker-registry.com:8000/official）作为 mirror。\n');

  const rl = null;
  try {
    const baseRegistry = await prompt(
      rl,
      '  Base registry',
      initialBase || '',
      validateBaseRegistry,
    );

    console.log('\n2. Builder 镜像（rust / java / maven / node 等编译环境）');
    console.log('   默认从 zhcoder-docker-registry.com:8000 拉取。切换到其他私有仓库时，');
    console.log('   输入 registry 地址（如 registry.my.com:5000）。\n');

    const builderRegistry = await prompt(
      rl,
      '  Builder registry',
      initialBuilder,
      validateBuilderRegistry,
    );

    console.log('\n3. crates.io 镜像（Rust crate 拉取来源）');
    console.log('   留空 = 官方 crates.io（crates.io）；');
    console.log(`   输入镜像 URL 作为 [source.crates-io] 的 replace-with。`);
    console.log(`   example: ${CRATE_REGISTRY_EXAMPLE}（自动补 sparse+ 与 /api/v1/crates/）`);
    console.log('   也接受完整 URL，如 sparse+https://rsproxy.cn/index/。\n');

    const crateRegistry = await prompt(
      rl,
      '  crates.io mirror',
      initialCrate || '',
      validateCrateRegistry,
    );

    console.log('\n4. crates.io / mirror 的 auth token');
    console.log('   留空 = 不写 token（公开镜像如官方 crates.io / rsproxy 不需要）。');
    console.log('   私有 kellnr 等需认证的镜像，输入 token（明文输入）。\n');

    const crateToken = await promptVisible(rl, '  crates.io token', initialCrateToken, validateCrateToken);

    console.log('\n5. 私有 crate registry（托管 libs/* + sdk/rust/* 的发布 + 拉取）');
    console.log('   留空 = 无私有 registry（无需 cargo publish --registry）');
    console.log('   填 URL（如 kellnr / 自建 nexus / cloudSmith）用于私有 crate');
    console.log('   与上面 crates-io 镜像独立——可以是同一个 registry（kellnr 同时配 upstream）');
    console.log(`   默认 example: ${KELLNR_REGISTRY.replace(/^sparse\+/, '')}`);
    console.log('   自动补 sparse+ 与 /api/v1/crates/\n');

    const initialPrivate = (() => {
      const v = existing?.privateCrateRegistry;
      if (v) return v;
      // 向后兼容：旧 crateRegistry 等于 kellnr 时自动迁移到新字段
      if (initialCrate && normalizeCrateRegistry(initialCrate) === KELLNR_REGISTRY) return initialCrate;
      return '';
    })();

    const privateCrateRegistry = await prompt(
      rl,
      '  private crate registry',
      initialPrivate,
      validateCrateRegistry,
    );

    console.log('\n6. 私有 crate registry 的 auth token');
    console.log('   留空 = 无认证（公开匿名 registry）');
    console.log('   私有 kellnr 等需认证 registry，输入 token（明文输入）');
    console.log('   用于 cargo publish --registry privateCrateRegistry\n');

    const privateCrateRegistryToken = await promptVisible(rl, '  private crate token', initialPrivateCrateToken, validateCrateToken);

    console.log('\n7. Maven 镜像（Java 依赖拉取来源）');
    console.log('   留空 = 官方 Maven Central（repo.maven.apache.org）；');
    console.log(`   输入镜像 URL 作为 <mirror> 替换 central。example: ${MVN_REGISTRY_EXAMPLE}`);
    console.log('   生成 .mvn/settings.xml + .mvn/maven.config（mvn 自动加载）。\n');

    const mavenRegistry = await prompt(
      rl,
      '  maven mirror',
      initialMaven || '',
      validateMavenRegistry,
    );

    console.log('\n6. Maven 认证方式（拉取 mirror + mvn deploy 发布都用此认证）');
    console.log('   none     = 不写 <servers>（公开 mirror 如 Maven Central 无需认证）');
    console.log('   token    = 公网发布（如 Maven Central project token；username 固定 "token"）');
    console.log('   userpass = 私有部署（reposilite / nexus 的 username + password）\n');

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
      console.log('\n   输入 token（公网 project token，明文输入）。');
      console.log('   直接回车 = 保留 existing token（若有）。\n');
      mavenToken = await promptVisible(rl, '  maven token', initialMavenToken, validateCrateToken);
      mavenUsername = '';
      mavenPassword = '';
    } else if (mavenAuthMode === 'userpass') {
      console.log('\n   输入 username + password（私有 reposilite / nexus，明文输入）。');
      console.log('   直接回车 = 保留 existing 值（若有）。\n');
      mavenUsername = await promptVisible(rl, '  maven username', initialMavenUsername, validateCrateToken);
      mavenPassword = await promptVisible(rl, '  maven password', initialMavenPassword, validateCrateToken);
      mavenToken = '';
    } else {
      mavenToken = '';
      mavenUsername = '';
      mavenPassword = '';
    }

    console.log('\n7. npm 镜像（Node 依赖拉取来源）');
    console.log('   留空 = 官方 npmjs.com（registry.npmjs.org）；');
    console.log(`   输入镜像 URL 作为 registry。example: ${NPM_REGISTRY_EXAMPLE}`);
    console.log('   生成项目根 .npmrc（npm/pnpm 自动加载）。\n');

    const npmRegistry = await prompt(
      rl,
      '  npm mirror',
      initialNpm || '',
      validateNpmRegistry,
    );

    console.log('\n8. npm mirror 的 auth token');
    console.log('   留空 = 不写 _authToken；');
    console.log('   私有 verdaccio/nexus 等需认证的镜像，输入 token（写入 //host/:_authToken，明文输入）。\n');

    const npmToken = await promptVisible(rl, '  npm token', initialNpmToken, validateCrateToken);

    console.log('\n9. 同步到 .gitlab-ci.yml');
    console.log('   将配置写入 .gitlab-ci.yml 的 variables: 段（CODER_* 变量）。');
    console.log('   非敏感值（registry URL 等）直接写明文；');
    console.log('   敏感值（token / password）引用 GitLab CI Variables（如 $CRATE_TOKEN）。');
    console.log('   yes = 同步（需在 GitLab Settings -> CI/CD -> Variables 创建对应变量）\n');

    const syncInput = await prompt(
      rl,
      '  sync to .gitlab-ci.yml',
      'no',
      validateYesNo,
    );
    const syncGitlabCI = syncInput === 'yes' || syncInput === 'y';

    return {
      cfg: {
        baseRegistry,
        builderRegistry,
        crateRegistry,
        crateToken,
        privateCrateRegistry,
        privateCrateRegistryToken,
        mavenRegistry,
        mavenAuthMode,
        mavenToken,
        mavenUsername,
        mavenPassword,
        npmRegistry,
        npmToken,
      },
      syncGitlabCI,
      source: 'interactive prompt',
    };
  } finally {
    void rl;
  }
}

function renderCargoConfig(crateUrl, privateUrl) {
  // cargo 不允许两个 source 指向同一 registry URL。同 URL 时（典型场景：kellnr 同时
  // 充当 crates-io 镜像 + 私有 registry）合并成单个 source 用 kellnr 作 name，避免冲突。
  // 4 种组合：
  //   A. 都空                          → 仅 crates-io，无 replace-with
  //   B. 仅 crate                      → crates-io 替换为 crate mirror
  //   C. 仅 private                    → crates-io 不替换；额外声明 kellnr 私有 source
  //   D. 都有且 URL 不同               → crates-io 替换 + 独立 kellnr 私有 source
  const isSame = crateUrl && privateUrl && crateUrl === privateUrl;
  const crateMirrorName = (crateUrl && isSame) ? 'kellnr' : MIRROR_NAME;
  const lines = [];

  if (crateUrl) {
    lines.push('[source.crates-io]');
    lines.push(`replace-with = "${crateMirrorName}"`);
    lines.push('');
    lines.push(`[source.${crateMirrorName}]`);
    lines.push(`registry = "${crateUrl}"`);
  } else {
    lines.push('[source.crates-io]');
    lines.push('# 官方 crates.io，无 replace-with');
  }

  if (privateUrl && !isSame) {
    lines.push('');
    lines.push('# 私有 crate registry - 托管 libs/* 和 sdk/rust/* 发布的 crate');
    lines.push('# cargo publish --registry kellnr  发布到此处');
    lines.push('# cargo add <crate> --registry kellnr  从此处拉取');
    lines.push('[source.kellnr]');
    lines.push(`registry = "${privateUrl}"`);
  }

  lines.push('');
  lines.push('[net]');
  lines.push('git-fetch-with-cli = true');
  lines.push('');
  return lines.join('\n');
}

function renderCargoCredentials(crateUrl, crateToken, privateUrl, privateToken) {
  const sections = [];
  const isSame = crateUrl && privateUrl && crateUrl === privateUrl;
  const header = '# 由 scripts/setup-wizard.js 生成；已在 .gitignore，请勿 commit。';

  if (isSame) {
    if (crateToken || privateToken) {
      sections.push(`[registry.kellnr]`);
      sections.push(`token = "${crateToken || privateToken}"`);
    }
  } else {
    if (crateUrl && crateToken) {
      sections.push(`[registry.${MIRROR_NAME}]`);
      sections.push(`token = "${crateToken}"`);
    }
    if (privateUrl && privateToken) {
      sections.push(`[registry.kellnr]`);
      sections.push(`token = "${privateToken}"`);
    }
  }

  if (sections.length === 0) return null;
  return [header, ...sections, ''].join('\n');
}

function applyCargoConfig(cfg) {
  fs.mkdirSync(CARGO_DIR, { recursive: true });
  const crateUrl = normalizeCrateRegistry(cfg.crateRegistry);
  const privateUrl = normalizeCrateRegistry(cfg.privateCrateRegistry);

  fs.writeFileSync(CARGO_CONFIG_PATH, renderCargoConfig(crateUrl, privateUrl));

  const credentialsContent = renderCargoCredentials(
    crateUrl, cfg.crateToken,
    privateUrl, cfg.privateCrateRegistryToken,
  );
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

// 生成需要写入 .gitlab-ci.yml variables 段的 CODER_* 行。
// 空值字段跳过；sensitive 字段写 "$CI_VAR" 引用，非 sensitive 写明文。
function generateCILines(cfg) {
  const lines = ['  # --- setup-wizard 同步（敏感值引用 GitLab CI Variables）---'];
  for (const m of CI_FIELD_MAP) {
    const val = cfg[m.field];
    if (!val) continue;
    if (m.sensitive) {
      lines.push(`  ${m.var}: "$${m.ciVar}"`);
    } else {
      lines.push(`  ${m.var}: "${val}"`);
    }
  }
  return lines;
}

// 修改 .gitlab-ci.yml 的 variables: 段，替换其中所有 CODER_* 行。
// 保留段内其他变量和注释；用逐行处理避免 YAML 序列化丢格式。
function applyGitlabCI(cfg) {
  if (!fs.existsSync(GITLAB_CI_PATH)) {
    console.log(chalk.yellow(`[wizard] .gitlab-ci.yml not found, skipping CI sync.`));
    return false;
  }

  const content = fs.readFileSync(GITLAB_CI_PATH, 'utf8');
  const lines = content.split('\n');

  // 定位 variables: 段范围（行首 variables: 到下一个行首非空行）
  let varStart = -1;
  let varEnd = lines.length;
  for (let i = 0; i < lines.length; i++) {
    if (/^variables:\s*$/.test(lines[i])) {
      varStart = i;
    } else if (varStart >= 0 && /^\S/.test(lines[i])) {
      varEnd = i;
      break;
    }
  }

  if (varStart < 0) {
    console.log(chalk.yellow(`[wizard] no "variables:" section in .gitlab-ci.yml, skipping CI sync.`));
    return false;
  }

  // 保留 variables 段内非 CODER_ 的行（含注释和空行）
  const varLines = lines.slice(varStart + 1, varEnd);
  const kept = varLines.filter((l) => !/^\s+CODER_/.test(l) && !/^\s+# --- setup-wizard/.test(l));

  const newLines = [
    ...lines.slice(0, varStart + 1),
    ...kept,
    ...generateCILines(cfg),
    ...lines.slice(varEnd),
  ];

  fs.writeFileSync(GITLAB_CI_PATH, newLines.join('\n') + '\n');
  return true;
}

// 返回 wizard 需要提示用户在 GitLab 中创建的 CI Variable 名列表。
function requiredCIVariables(cfg) {
  const names = [];
  for (const m of CI_FIELD_MAP) {
    if (m.sensitive && cfg[m.field]) {
      names.push(m.ciVar);
    }
  }
  return names;
}

async function main() {
  const args = parseArgs(process.argv);

  if (args.show) {
    showCurrent();
    return;
  }

  const existing = readExistingConfig();

  const isTTY = process.stdin.isTTY === true;
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

  let ciSynced = false;
  if (result.syncGitlabCI) {
    ciSynced = applyGitlabCI(result.cfg);
  }

  showSummary(result.cfg, result.source, ciSynced);
}

main().catch((err) => {
  console.error(chalk.red(`[wizard] crashed: ${err.message}`));
  if (err.stack) console.error(err.stack);
  process.exit(1);
});
