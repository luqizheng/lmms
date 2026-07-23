'use strict';

const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');

const { ROOT } = require('./utils.js');
const { chalk } = require('./ui.js');

const CONFIG_PATH = path.join(ROOT, 'infrastructure', '.docker-registry.json');
const M2_SETTINGS_PATH = path.join(os.homedir(), '.m2', 'settings.xml');
const M2_REPO_ROOT = path.join(os.homedir(), '.m2', 'repository');
const GROUP_PATH = 'com/kudi/coder';
const VALID_PROFILES = new Set(['java8', 'java17', 'java21']);

const DEFAULT_REPO_ID = {
  release: 'reposilite-repository-releases',
  snapshot: 'reposilite-repository-snapshots',
};
// 与旧 .sh 保持一致：无 wizard 且无 --repo-url / MAVEN_REPO_URL 时的 fallback。
const DEFAULT_REPO_URL = {
  release: 'http://192.168.2.12:18181/releases',
  snapshot: 'http://192.168.2.12:18181/snapshots',
};

function parseArgs(argv) {
  const out = {
    profile: 'java8',
    modules: '',
    user: '',
    pass: '',
    repoId: '',
    repoUrl: '',
    dryRun: false,
    useMvn: false,
    extraMvn: [],
    help: false,
  };
  const args = argv.slice(2);
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    switch (a) {
      case '--profile': out.profile = args[++i]; break;
      case '--modules': out.modules = args[++i]; break;
      case '--user': out.user = args[++i]; break;
      case '--pass': out.pass = args[++i]; break;
      case '--repo-id': out.repoId = args[++i]; break;
      case '--repo-url': out.repoUrl = args[++i]; break;
      case '--mvn': out.useMvn = true; break;
      case '--dry-run': out.dryRun = true; break;
      case '--': out.extraMvn = args.slice(i + 1); i = args.length; break;
      case '-h': case '--help': out.help = true; break;
      default:
        throw new Error(`Unknown argument: ${a}`);
    }
  }
  if (!VALID_PROFILES.has(out.profile)) {
    throw new Error(`--profile must be one of: ${[...VALID_PROFILES].join(', ')}`);
  }
  return out;
}

function readWizardConfig() {
  if (!fs.existsSync(CONFIG_PATH)) return null;
  try {
    const raw = JSON.parse(fs.readFileSync(CONFIG_PATH, 'utf8'));
    return {
      mavenRegistry: raw.mavenRegistry || '',
      mavenAuthMode: raw.mavenAuthMode || 'none',
      mavenToken: raw.mavenToken || '',
      mavenUsername: raw.mavenUsername || '',
      mavenPassword: raw.mavenPassword || '',
    };
  } catch (err) {
    console.log(chalk.yellow(`[publish] warning: failed to parse ${CONFIG_PATH}: ${err.message}`));
    return null;
  }
}

// reposilite 惯例：<base>/releases 与 <base>/snapshots。用户结构不同请用 --repo-url 覆盖。
function deriveRepoUrl(baseUrl, variant) {
  const trimmed = baseUrl.replace(/\/+$/, '');
  return `${trimmed}/${variant === 'release' ? 'releases' : 'snapshots'}`;
}

// 凭证优先级：CLI --user/--pass > env MAVEN_USERNAME/MAVEN_PASSWORD > wizard config > ~/.m2/settings.xml
function resolveCredentials(args, wizard, repoId) {
  if (args.user || args.pass) {
    if (!args.user || !args.pass) {
      throw new Error('both --user and --pass must be provided');
    }
    return { source: 'CLI --user/--pass', user: args.user, pass: args.pass };
  }
  if (process.env.MAVEN_USERNAME && process.env.MAVEN_PASSWORD) {
    return {
      source: 'env MAVEN_USERNAME/MAVEN_PASSWORD',
      user: process.env.MAVEN_USERNAME,
      pass: process.env.MAVEN_PASSWORD,
    };
  }
  if (wizard && wizard.mavenAuthMode === 'userpass' && wizard.mavenUsername && wizard.mavenPassword) {
    return {
      source: 'wizard config (userpass)',
      user: wizard.mavenUsername,
      pass: wizard.mavenPassword,
    };
  }
  if (wizard && wizard.mavenAuthMode === 'token' && wizard.mavenToken) {
    return {
      source: 'wizard config (token; username fixed to "token")',
      user: 'token',
      pass: wizard.mavenToken,
    };
  }
  const fromXml = readSettingsXml(repoId);
  if (fromXml) {
    return { source: `~/.m2/settings.xml (id=${repoId})`, ...fromXml };
  }
  return null;
}

// 解析 ~/.m2/settings.xml，查找指定 <server> 的 username/password。
// 用字符串正则匹配（避免引入 XML parser 依赖）；xmlns 差异不影响这里的匹配。
function readSettingsXml(repoId) {
  if (!fs.existsSync(M2_SETTINGS_PATH)) return null;
  const xml = fs.readFileSync(M2_SETTINGS_PATH, 'utf8');
  // 逐个 <server>...</server> 块解析
  const re = /<server>([\s\S]*?)<\/server>/g;
  let m;
  while ((m = re.exec(xml)) !== null) {
    const block = m[1];
    const idMatch = /<id>\s*([^<]+?)\s*<\/id>/.exec(block);
    if (!idMatch || idMatch[1].trim() !== repoId) continue;
    const u = /<username>\s*([^<]*?)\s*<\/username>/.exec(block);
    const p = /<password>\s*([^<]*?)\s*<\/password>/.exec(block);
    if (u && p) return { user: u[1], pass: p[1] };
  }
  return null;
}

function writeTempSettingsXml(repoId, user, pass) {
  const tmp = path.join(os.tmpdir(), `reposilite-settings-${process.pid}-${Date.now()}.xml`);
  const xml = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<settings xmlns="http://maven.apache.org/SETTINGS/1.0.0">',
    '  <servers>',
    '    <server>',
    `      <id>${escapeXml(repoId)}</id>`,
    `      <username>${escapeXml(user)}</username>`,
    `      <password>${escapeXml(pass)}</password>`,
    '    </server>',
    '  </servers>',
    '</settings>',
    '',
  ].join('\n');
  fs.writeFileSync(tmp, xml);
  return tmp;
}

function escapeXml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

function readPomVersion() {
  const pomPath = path.join(ROOT, 'sdk', 'java', 'pom.xml');
  if (!fs.existsSync(pomPath)) throw new Error(`sdk/java/pom.xml not found`);
  const xml = fs.readFileSync(pomPath, 'utf8');
  // 抓 <project> 直接子级的第一个 <version>（跳过 <parent><version>）
  const m = /<project\b[\s\S]*?<\/project>/.exec(xml);
  if (!m) throw new Error('malformed pom.xml (no <project>)');
  const projBlock = m[0].replace(/<parent>[\s\S]*?<\/parent>/g, '');
  const v = /<version>\s*([^<]+?)\s*<\/version>/.exec(projBlock);
  if (!v) throw new Error('could not find <version> in sdk/java/pom.xml');
  return v[1].trim();
}

function runMvn(mvnArgs, sdkDir) {
  console.log(`[mvn] ${mvnArgs.join(' ')}`);
  const res = spawnSync('mvn', mvnArgs, { cwd: sdkDir, stdio: 'inherit', shell: false });
  return res.status === 0;
}

// curl -T 上传 + 校验和文件（md5/sha1/sha256）。release 模式先 HEAD 检查已存在则 SKIP。
function uploadFile(filePath, remoteBase, relPath, curlAuth, skipIfExists) {
  const suffixes = ['', '.md5', '.sha1', '.sha256'];
  const buf = fs.readFileSync(filePath);
  const hashes = {
    md5: crypto.createHash('md5').update(buf).digest('hex'),
    sha1: crypto.createHash('sha1').update(buf).digest('hex'),
    sha256: crypto.createHash('sha256').update(buf).digest('hex'),
  };
  for (const suffix of suffixes) {
    const remote = `${remoteBase}/${relPath}${suffix}`;
    if (skipIfExists) {
      const head = spawnSync('curl', ['-sf', '-u', curlAuth, '-o', '/dev/null', remote], { stdio: 'ignore' });
      if (head.status === 0) {
        console.log(`  SKIP ${relPath}${suffix}  (already exists)`);
        continue;
      }
    }
    let uploadArgs;
    if (suffix === '') {
      uploadArgs = ['-sf', '-u', curlAuth, '-T', filePath, '-o', '/dev/null', remote];
    } else {
      const algo = suffix.slice(1);
      const tmp = path.join(os.tmpdir(), `pub-${process.pid}-${Date.now()}${suffix}`);
      fs.writeFileSync(tmp, hashes[algo]);
      uploadArgs = ['-sf', '-u', curlAuth, '-T', tmp, '-o', '/dev/null', remote];
      try {
        const up = spawnSync('curl', uploadArgs, { stdio: 'inherit' });
        if (up.status !== 0) { fs.unlinkSync(tmp); return { ok: false, path: `${relPath}${suffix}` }; }
      } finally { try { fs.unlinkSync(tmp); } catch (_) { /* best-effort */ } }
      console.log(`  OK   ${relPath}${suffix}`);
      continue;
    }
    const up = spawnSync('curl', uploadArgs, { stdio: 'inherit' });
    if (up.status !== 0) return { ok: false, path: `${relPath}${suffix}` };
    console.log(`  OK   ${relPath}${suffix}`);
  }
  return { ok: true };
}

// 递归找 M2_REPO_ROOT/com/kudi/coder/*/<VERSION>/*.pom + *.jar，排序稳定。
function findArtifacts(version) {
  const base = path.join(M2_REPO_ROOT, GROUP_PATH);
  if (!fs.existsSync(base)) return [];
  const out = [];
  const stack = [base];
  while (stack.length) {
    const dir = stack.pop();
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    for (const e of entries) {
      const full = path.join(dir, e.name);
      if (e.isDirectory()) { stack.push(full); continue; }
      if (!e.isFile()) continue;
      if (!full.split(path.sep).includes(version)) continue;
      const name = e.name;
      if (name === '_remote.repositories') continue;
      if (/\.(md5|sha1|sha256)$/.test(name)) continue;
      if (!/\.(pom|jar)$/.test(name)) continue;
      out.push(full);
    }
  }
  return out.sort();
}

function toRelPath(fullPath) {
  return path.relative(M2_REPO_ROOT, fullPath).split(path.sep).join('/');
}

function requireBinary(name, hint) {
  const res = spawnSync(name, ['--version'], { stdio: 'ignore' });
  if (res.error) {
    throw new Error(`'${name}' not found on PATH. ${hint || ''}`.trim());
  }
}

// variant: 'release' | 'snapshot'
function runPublish(variant, argv) {
  if (variant !== 'release' && variant !== 'snapshot') {
    throw new Error(`invalid variant: ${variant}`);
  }
  let args;
  try { args = parseArgs(argv); }
  catch (err) { console.error(chalk.red(`ERROR: ${err.message}`)); process.exit(2); }

  if (args.help) { printHelp(variant); process.exit(0); }

  requireBinary('mvn', "Install Maven 3.9+.");
  if (!args.useMvn) requireBinary('curl', "Required for default upload mode.");

  const version = readPomVersion();
  const isSnapshot = version.endsWith('-SNAPSHOT');
  if (variant === 'release' && isSnapshot) {
    console.error(chalk.red(`ERROR: sdk/java/pom.xml version is '${version}' (a SNAPSHOT).`));
    console.log(`       The release repository rejects -SNAPSHOT artifacts.`);
    console.log(`       Bump the pom to a release version first, e.g.:`);
    console.log(`         mvn -f sdk/java/pom.xml versions:set -DnewVersion=<release-ver> versions:commit`);
    process.exit(3);
  }
  if (variant === 'snapshot' && !isSnapshot) {
    console.error(chalk.red(`ERROR: sdk/java/pom.xml version is '${version}' (not a SNAPSHOT).`));
    console.log(`       The snapshot repository rejects non-SNAPSHOT artifacts.`);
    console.log(`       Bump the pom to a snapshot version first, e.g.:`);
    console.log(`         mvn -f sdk/java/pom.xml versions:set -DnewVersion=${version}-SNAPSHOT versions:commit`);
    process.exit(3);
  }

  const wizard = readWizardConfig();

  // repo id
  const repoId = args.repoId || process.env.MAVEN_REPO_ID || DEFAULT_REPO_ID[variant];

  // repo url 优先级: --repo-url > env MAVEN_REPO_URL > wizard mavenRegistry+suffix > 硬编码默认
  let repoUrl;
  let repoUrlSource;
  if (args.repoUrl) { repoUrl = args.repoUrl; repoUrlSource = 'CLI --repo-url'; }
  else if (process.env.MAVEN_REPO_URL) { repoUrl = process.env.MAVEN_REPO_URL; repoUrlSource = 'env MAVEN_REPO_URL'; }
  else if (wizard && wizard.mavenRegistry) {
    repoUrl = deriveRepoUrl(wizard.mavenRegistry, variant);
    repoUrlSource = `wizard mavenRegistry + /${variant === 'release' ? 'releases' : 'snapshots'}`;
  } else {
    repoUrl = DEFAULT_REPO_URL[variant];
    repoUrlSource = 'default (hardcoded fallback)';
  }

  const creds = resolveCredentials(args, wizard, repoId);

  const mode = args.useMvn ? 'mvn-deploy' : 'curl-upload (default)';
  console.log(chalk.green(`==> Repository : ${repoId} @ ${repoUrl}`));
  console.log(`==> URL source : ${repoUrlSource}`);
  console.log(`==> Version    : ${version}  (from sdk/java/pom.xml)`);
  console.log(`==> Profile    : ${args.profile}`);
  console.log(`==> Modules    : ${args.modules || '<all>'}`);
  console.log(`==> Mode       : ${mode}`);
  console.log(`==> Dry run    : ${args.dryRun ? 'yes' : 'no'}`);
  console.log(`==> Credentials: ${creds ? creds.source : '(none)'}`);
  console.log('');

  // 若有凭证，生成临时 settings.xml（用于 mvn deploy 模式；curl 模式则直接用 user:pass）
  let settingsFile = null;
  const settingsArgs = [];
  if (creds && args.useMvn) {
    settingsFile = writeTempSettingsXml(repoId, creds.user, creds.pass);
    settingsArgs.push('-s', settingsFile);
  }
  const cleanup = () => { if (settingsFile) { try { fs.unlinkSync(settingsFile); } catch (_) { /* best-effort */ } } };
  process.on('exit', cleanup);

  const sdkDir = path.join(ROOT, 'sdk', 'java');

  try {
    if (args.dryRun) {
      console.log('==> DRY RUN: running clean verify only.');
      const ok = runMvn([`-P${args.profile}`, ...settingsArgs, '-DskipTests', 'clean', 'verify'], sdkDir);
      if (!ok) process.exit(4);
      console.log(chalk.green('==> Dry run OK.'));
      return;
    }

    if (args.useMvn) {
      const alt = `${repoId}::default::${repoUrl}`;
      const mvnArgs = [
        `-P${args.profile}`, ...settingsArgs, '-DskipTests',
        `-DaltDeploymentRepository=${alt}`,
        `-DaltSnapshotDeploymentRepository=${alt}`,
      ];
      if (args.modules) mvnArgs.push('-pl', args.modules, '-am');
      if (args.extraMvn.length) mvnArgs.push(...args.extraMvn);
      const ok = runMvn([...mvnArgs, 'clean', 'deploy'], sdkDir);
      if (!ok) { console.error(chalk.red(`ERROR: mvn deploy failed.`)); process.exit(5); }
      console.log(chalk.green(`==> ${variant} deploy complete (mvn mode).`));
      return;
    }

    // ---------- curl mode ----------
    console.log(chalk.green('==> Step 1: mvn install (compile + package to ~/.m2)'));
    const installArgs = [`-P${args.profile}`, ...settingsArgs, '-DskipTests', 'clean', 'install'];
    if (args.modules) installArgs.push('-pl', args.modules, '-am');
    if (args.extraMvn.length) installArgs.push(...args.extraMvn);
    if (!runMvn(installArgs, sdkDir)) { console.error(chalk.red('ERROR: mvn install failed.')); process.exit(4); }

    if (!creds) {
      console.error(chalk.red('ERROR: no credentials available for curl upload.'));
      console.log('       Use --user/--pass, MAVEN_USERNAME/MAVEN_PASSWORD env, wizard config, or ~/.m2/settings.xml.');
      process.exit(5);
    }
    const curlAuth = `${creds.user}:${creds.pass}`;

    console.log('');
    console.log(chalk.green(`==> Step 2: curl -T upload to ${repoUrl}`));

    const files = findArtifacts(version);
    if (files.length === 0) {
      console.error(chalk.red(`ERROR: no artifacts found under ${M2_REPO_ROOT}/${GROUP_PATH}/**/${version}/`));
      process.exit(6);
    }

    // release 模式：远端已存在则 SKIP；snapshot 模式：允许覆写（快照 policy）
    const skipIfExists = variant === 'release';

    let total = 0, fail = 0;
    for (const file of files) {
      const rel = toRelPath(file);
      const r = uploadFile(file, repoUrl, rel, curlAuth, skipIfExists);
      total++;
      if (!r.ok) {
        fail++;
        console.error(chalk.red(`  FAIL ${r.path}`));
        console.log(`  -> stopping at ${rel}`);
        break;
      }
    }

    console.log('');
    console.log(`==> Upload complete. Artifacts: ${total}, Failures: ${fail}`);
    if (fail > 0) process.exit(6);
    console.log(chalk.green(`==> ${variant} deploy complete (curl mode).`));
    console.log(`    Repository: ${repoUrl}`);
    console.log(`    Version   : ${version}`);
  } finally {
    cleanup();
  }
}

function printHelp(variant) {
  const bin = `scripts/publish-maven-${variant}.js`;
  const versionPolicy = variant === 'release'
    ? 'Release artifacts MUST NOT carry the -SNAPSHOT suffix.'
    : 'Snapshot artifacts MUST carry the -SNAPSHOT suffix.';
  const defaultUrl = DEFAULT_REPO_URL[variant];
  console.log(`Publish Java SDK artifacts to the Reposilite ${variant.toUpperCase()} repository.

Version policy:
  ${versionPolicy}

Two upload modes:
  curl (default): mvn install -> curl -T upload from ~/.m2 to Reposilite.
                  Works around Maven Aether HttpClient's Expect:100-continue
                  incompatibility with Reposilite (causes HTTP 400).
  mvn  (--mvn):   Direct mvn deploy with -DaltDeploymentRepository.
                  Use only if Reposilite is fixed for the issue.

Configuration sources (highest first):
  Repo URL:  --repo-url > env MAVEN_REPO_URL >
             wizard mavenRegistry + /${variant === 'release' ? 'releases' : 'snapshots'} >
             hardcoded default (${defaultUrl})
  Repo ID:   --repo-id  > env MAVEN_REPO_ID  > default (${DEFAULT_REPO_ID[variant]})
  Credentials: --user/--pass > env MAVEN_USERNAME/MAVEN_PASSWORD >
             wizard config (authMode=userpass or token) > ~/.m2/settings.xml (by id)

Usage:
  node ${bin}
  node ${bin} --profile java17
  node ${bin} --modules "common,user-service-user-sdk"
  node ${bin} --user admin --pass 's3cret'
  node ${bin} --mvn                     # force mvn deploy mode
  node ${bin} --dry-run                 # clean verify only, no deploy
  node ${bin} -- -pl user-service-user-sdk -am  # extra mvn args
`);
}

module.exports = { runPublish };
