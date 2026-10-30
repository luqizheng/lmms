'use strict';

// Java SDK publishing to Reposilite. Absorbed from the retired maven-publish lib.
// Default upload mode stays curl -T (works around Reposilite's Expect:100-continue
// incompatibility); --mvn keeps the direct-deploy branch. Root/homedir/env/exec
// are injectable so the whole flow is unit-testable.
// Legacy script exit codes (3..8) collapse to the CLI exit table: usage → 2,
// unmet precondition → 3, external tool failure → 4 (spec: 2026-10-30 design §Global).

const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
const { UsageError, GuardError, ExternalError } = require('./errors.js');

const DEFAULT_REPO_ID = {
  release: 'reposilite-repository-releases',
  snapshot: 'reposilite-repository-snapshots',
};
// Same fallbacks as the retired .sh lineage: no wizard config and no --repo-url/MAVEN_REPO_URL.
const DEFAULT_REPO_URL = {
  release: 'http://192.168.2.12:18181/releases',
  snapshot: 'http://192.168.2.12:18181/snapshots',
};
const GROUP_PATH = 'io/digua';
const VALID_PROFILES = new Set(['java8', 'java17', 'java21']);

function validateMavenVersion({ version, kind }) {
  const isSnapshot = String(version).endsWith('-SNAPSHOT');
  if (kind === 'release' && isSnapshot) {
    throw new UsageError('MAVEN_VERSION_SNAPSHOT', `--version '${version}' carries -SNAPSHOT; the releases repository rejects it`);
  }
  if (kind === 'snapshot' && !isSnapshot) {
    throw new UsageError('MAVEN_VERSION_NOT_SNAPSHOT', `--version '${version}' lacks -SNAPSHOT; the snapshots repository requires it`);
  }
}

function readWizardConfig(root, fsMod = fs) {
  const configPath = path.join(root, 'infrastructure', '.docker-registry.json');
  if (!fsMod.existsSync(configPath)) return null;
  try {
    const raw = JSON.parse(fsMod.readFileSync(configPath, 'utf8'));
    return {
      mavenRegistry: raw.mavenRegistry || '',
      mavenAuthMode: raw.mavenAuthMode || 'none',
      mavenToken: raw.mavenToken || '',
      mavenUsername: raw.mavenUsername || '',
      mavenPassword: raw.mavenPassword || '',
    };
  } catch {
    return null;
  }
}

function deriveRepoUrl(baseUrl, variant) {
  const trimmed = String(baseUrl).replace(/\/+$/, '');
  return `${trimmed}/${variant === 'release' ? 'releases' : 'snapshots'}`;
}

// Priority: CLI --user/--pass > env MAVEN_USERNAME/MAVEN_PASSWORD > wizard config > ~/.m2/settings.xml
function resolveCredentials({ args, env = process.env, wizard, repoId, settingsXml }) {
  if (args.user || args.pass) {
    if (!args.user || !args.pass) throw new UsageError('MAVEN_CREDENTIALS_PARTIAL', 'both --user and --pass must be provided');
    return { source: 'CLI --user/--pass', user: args.user, pass: args.pass };
  }
  if (env.MAVEN_USERNAME && env.MAVEN_PASSWORD) {
    return { source: 'env MAVEN_USERNAME/MAVEN_PASSWORD', user: env.MAVEN_USERNAME, pass: env.MAVEN_PASSWORD };
  }
  if (wizard && wizard.mavenAuthMode === 'userpass' && wizard.mavenUsername && wizard.mavenPassword) {
    return { source: 'wizard config (userpass)', user: wizard.mavenUsername, pass: wizard.mavenPassword };
  }
  if (wizard && wizard.mavenAuthMode === 'token' && wizard.mavenToken) {
    return { source: 'wizard config (token; username fixed to "token")', user: 'token', pass: wizard.mavenToken };
  }
  if (settingsXml) {
    const fromXml = findServerInSettings(settingsXml, repoId);
    if (fromXml) return { source: `~/.m2/settings.xml (id=${repoId})`, ...fromXml };
  }
  return null;
}

// Regex-based <server> parsing (no XML parser dependency; xmlns differences don't matter).
function findServerInSettings(xml, repoId) {
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

function escapeXml(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&apos;');
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
  fs.writeFileSync(tmp, xml, 'utf-8');
  return tmp;
}

function readPomVersion(root, fsMod = fs) {
  const pomPath = path.join(root, 'sdk', 'java', 'pom.xml');
  if (!fsMod.existsSync(pomPath)) throw new GuardError('MAVEN_POM_MISSING', 'sdk/java/pom.xml not found', { pomPath });
  const xml = fsMod.readFileSync(pomPath, 'utf8');
  const projBlock = xml.replace(/<parent>[\s\S]*?<\/parent>/g, '');
  const v = /<version>\s*([^<]+?)\s*<\/version>/.exec(projBlock);
  if (!v) throw new GuardError('MAVEN_POM_MALFORMED', 'could not find <version> in sdk/java/pom.xml');
  return v[1].trim();
}

// Idempotent: only runs versions:set when the pom disagrees. Dry-run prints the plan.
function bumpPomVersion(root, targetVersion, { dryRun = false, exec, out, fsMod = fs } = {}) {
  const pomPath = path.join(root, 'sdk', 'java', 'pom.xml');
  const current = readPomVersion(root, fsMod);
  if (current === targetVersion) {
    out.log(`[bump] pom.xml <version> already ${targetVersion} — no change`);
    return;
  }
  out.log(`[bump] pom.xml <version>: ${current} -> ${targetVersion}`);
  if (dryRun) {
    out.log(`[bump]   (dry-run) would run: mvn versions:set -DnewVersion=${targetVersion} versions:commit`);
    return;
  }
  const ok1 = exec('mvn', ['-f', pomPath, 'versions:set', `-DnewVersion=${targetVersion}`, 'versions:commit'], { cwd: root });
  if (ok1.error || (ok1.status ?? 1) !== 0) {
    throw new ExternalError(`mvn versions:set failed (bump pom.xml <version> to ${targetVersion})`);
  }
  const after = readPomVersion(root, fsMod);
  if (after !== targetVersion) {
    throw new ExternalError(`pom.xml <version> bump mismatch: expected ${targetVersion}, got ${after} — inspect sdk/java/pom.xml and child <parent><version> blocks`);
  }
  out.log(`[bump] pom.xml now at <version>${targetVersion}</version>`);
}

function requireBinary(name, hint, exec) {
  const res = exec(name, ['--version'], { stdio: 'ignore' });
  if (res.error) throw new GuardError('MAVEN_BINARY_MISSING', `'${name}' not found on PATH. ${hint || ''}`.trim());
}

function findArtifacts(m2RepoRoot, version, fsMod = fs) {
  const base = path.join(m2RepoRoot, GROUP_PATH);
  if (!fsMod.existsSync(base)) return [];
  const out = [];
  const stack = [base];
  while (stack.length) {
    const dir = stack.pop();
    let entries;
    try {
      entries = fsMod.readdirSync(dir, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const e of entries) {
      const full = path.join(dir, e.name);
      if (e.isDirectory()) { stack.push(full); continue; }
      if (!full.split(path.sep).includes(version)) continue;
      if (e.name === '_remote.repositories') continue;
      if (/\.(md5|sha1|sha256)$/.test(e.name)) continue;
      if (!/\.(pom|jar)$/.test(e.name)) continue;
      out.push(full);
    }
  }
  return out.sort();
}

function toRelPath(m2RepoRoot, fullPath) {
  return path.relative(m2RepoRoot, fullPath).split(path.sep).join('/');
}

function uploadFile(filePath, remoteBase, relPath, curlAuth, skipIfExists, exec, fsMod = fs) {
  const suffixes = ['', '.md5', '.sha1', '.sha256'];
  const buf = fsMod.readFileSync(filePath);
  const hashes = {
    md5: crypto.createHash('md5').update(buf).digest('hex'),
    sha1: crypto.createHash('sha1').update(buf).digest('hex'),
    sha256: crypto.createHash('sha256').update(buf).digest('hex'),
  };
  for (const suffix of suffixes) {
    const remote = `${remoteBase}/${relPath}${suffix}`;
    if (skipIfExists) {
      const head = exec('curl', ['-sf', '-u', curlAuth, '-o', '/dev/null', remote], { stdio: 'ignore' });
      if ((head.status ?? 1) === 0) continue;
    }
    let uploadArgs;
    let tmp = null;
    if (suffix === '') {
      uploadArgs = ['-sf', '-u', curlAuth, '-T', filePath, '-o', '/dev/null', remote];
    } else {
      const algo = suffix.slice(1);
      tmp = path.join(os.tmpdir(), `coder-pub-${process.pid}-${Date.now()}${suffix}`);
      fsMod.writeFileSync(tmp, hashes[algo], 'utf-8');
      uploadArgs = ['-sf', '-u', curlAuth, '-T', tmp, '-o', '/dev/null', remote];
    }
    const up = exec('curl', uploadArgs, { stdio: 'inherit' });
    if (tmp) { try { fsMod.unlinkSync(tmp); } catch { /* best-effort */ } }
    if ((up.status ?? 1) !== 0) return { ok: false, path: `${relPath}${suffix}` };
  }
  return { ok: true };
}

// variant/kind: 'release' | 'snapshot'. curl mode: mvn install → curl -T from ~/.m2.
function publishMaven({
  root,
  version,
  kind = 'release',
  profile = 'java8',
  modules = '',
  user = '',
  pass = '',
  repoId = '',
  repoUrl = '',
  useMvn = false,
  dryRun = false,
  extraMvn = [],
  exec = (file, args, opts) => spawnSync(file, args, { ...opts, stdio: "inherit", shell: opts.shell ?? false }),
  env = process.env,
  m2RepoRoot = path.join(os.homedir(), '.m2', 'repository'),
  settingsPath = path.join(os.homedir(), '.m2', 'settings.xml'),
  out = { log: (m) => console.log(m), warn: (m) => console.warn(m) },
  fsMod = fs,
} = {}) {
  if (!version) throw new UsageError('MAVEN_VERSION_REQUIRED', '--version <ver> is required (e.g. 0.4.0 or 0.4.1-SNAPSHOT)');
  if (!VALID_PROFILES.has(profile)) {
    throw new UsageError('MAVEN_PROFILE_INVALID', `--profile must be one of: ${[...VALID_PROFILES].join(', ')}`);
  }
  validateMavenVersion({ version, kind });
  requireBinary('mvn', 'Install Maven 3.9+.', exec);
  if (!useMvn) requireBinary('curl', 'Required for default upload mode.', exec);

  bumpPomVersion(root, version, { dryRun, exec, out, fsMod });

  const wizard = readWizardConfig(root, fsMod);
  const effectiveRepoId = repoId || env.MAVEN_REPO_ID || DEFAULT_REPO_ID[kind];
  let effectiveUrl;
  let urlSource;
  if (repoUrl) { effectiveUrl = repoUrl; urlSource = 'CLI --repo-url'; }
  else if (env.MAVEN_REPO_URL) { effectiveUrl = env.MAVEN_REPO_URL; urlSource = 'env MAVEN_REPO_URL'; }
  else if (wizard && wizard.mavenRegistry) { effectiveUrl = deriveRepoUrl(wizard.mavenRegistry, kind); urlSource = `wizard mavenRegistry + /${kind === 'release' ? 'releases' : 'snapshots'}`; }
  else { effectiveUrl = DEFAULT_REPO_URL[kind]; urlSource = 'default (hardcoded fallback)'; }

  let settingsXml = null;
  try { settingsXml = fsMod.readFileSync(settingsPath, 'utf8'); } catch { /* absent */ }
  const creds = resolveCredentials({ args: { user, pass }, env, wizard, repoId: effectiveRepoId, settingsXml });

  out.log(`==> Repository : ${effectiveRepoId} @ ${effectiveUrl} (${urlSource})`);
  out.log(`==> Version    : ${version}  profile: ${profile}  dry-run: ${dryRun ? 'yes' : 'no'}`);
  out.log(`==> Credentials: ${creds ? creds.source : '(none)'}`);

  const settingsArgs = [];
  if (creds && useMvn) settingsArgs.push('-s', writeTempSettingsXml(effectiveRepoId, creds.user, creds.pass));

  try {
    const sdkDir = path.join(root, 'sdk', 'java');

    if (dryRun) {
      out.log('==> DRY RUN: running clean verify only.');
      const ok = exec('mvn', [`-P${profile}`, ...settingsArgs, '-DskipTests', 'clean', 'verify'], { cwd: sdkDir });
      if (ok.error || (ok.status ?? 1) !== 0) throw new ExternalError('dry-run mvn clean verify failed');
      return { dryRun: true, modules: 0, uploaded: 0, repoId: effectiveRepoId, repoUrl: effectiveUrl };
    }

    if (useMvn) {
      const alt = `${effectiveRepoId}::default::${effectiveUrl}`;
      const mvnArgs = [`-P${profile}`, ...settingsArgs, '-DskipTests', `-DaltDeploymentRepository=${alt}`, `-DaltSnapshotDeploymentRepository=${alt}`];
      if (modules) mvnArgs.push('-pl', modules, '-am');
      if (extraMvn.length) mvnArgs.push(...extraMvn);
      const ok = exec('mvn', [...mvnArgs, 'clean', 'deploy'], { cwd: sdkDir });
      if (ok.error || (ok.status ?? 1) !== 0) throw new ExternalError('mvn deploy failed');
      return { modules: 0, uploaded: 0, mode: 'mvn-deploy', repoId: effectiveRepoId, repoUrl: effectiveUrl };
    }

    out.log('==> Step 1: mvn install (compile + package to ~/.m2)');
    const installArgs = [`-P${profile}`, ...settingsArgs, '-DskipTests', 'clean', 'install'];
    if (modules) installArgs.push('-pl', modules, '-am');
    if (extraMvn.length) mvnExtra(extraMvn, installArgs);
    const okInstall = exec('mvn', installArgs, { cwd: sdkDir });
    if (okInstall.error || (okInstall.status ?? 1) !== 0) throw new ExternalError('mvn install failed');

    if (!creds) {
      throw new GuardError('MAVEN_CREDENTIALS_MISSING', 'no credentials available for curl upload — use --user/--pass, env, wizard config, or ~/.m2/settings.xml');
    }
    const curlAuth = `${creds.user}:${creds.pass}`;

    const files = findArtifacts(m2RepoRoot, version, fsMod);
    if (files.length === 0) {
      throw new GuardError('MAVEN_NO_ARTIFACTS', `no artifacts found under ${m2RepoRoot}/${GROUP_PATH}/**/${version}/`);
    }

    out.log(`==> Step 2: curl -T upload to ${effectiveUrl} (${files.length} artifacts)`);
    // releases skip already-existing remotes; snapshots allow overwrite (repo policy)
    const skipIfExists = kind === 'release';
    let uploaded = 0;
    for (const file of files) {
      const rel = toRelPath(m2RepoRoot, file);
      const r = uploadFile(file, effectiveUrl, rel, curlAuth, skipIfExists, exec, fsMod);
      if (!r.ok) throw new ExternalError(`upload failed at ${r.path} — stopping`);
      uploaded += 1;
    }
    return { modules: files.length, uploaded, repoId: effectiveRepoId, repoUrl: effectiveUrl };
  } finally {
    // temp settings files are timestamped; nothing durable to clean here
  }
}

function mvnExtra(extra, into) {
  into.push(...extra);
}

module.exports = { validateMavenVersion, resolveCredentials, deriveRepoUrl, readPomVersion, findArtifacts, publishMaven };
