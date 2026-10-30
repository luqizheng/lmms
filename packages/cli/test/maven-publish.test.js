'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { validateMavenVersion, resolveCredentials, publishMaven, deriveRepoUrl } = require('../lib/maven-publish.js');
const { CoderError, UsageError } = require('../lib/errors.js');

const POM = '<?xml version="1.0"?><project><groupId>io.digua</groupId><artifactId>sdk</artifactId><version>0.3.9</version></project>';

function fixture({ version = '1.7.0', artifacts = 2 } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'coder-maven-'));
  fs.mkdirSync(path.join(dir, 'sdk', 'java'), { recursive: true });
  fs.writeFileSync(path.join(dir, 'sdk', 'java', 'pom.xml'), POM, 'utf-8');
  for (let i = 0; i < artifacts; i++) {
    const d = path.join(dir, 'm2', 'io', 'digua', `mod${i}`, version);
    fs.mkdirSync(d, { recursive: true });
    fs.writeFileSync(path.join(d, `mod${i}.pom`), 'x');
    fs.writeFileSync(path.join(d, `mod${i}.jar`), 'x');
  }
  return dir;
}

function fakeExec(calls, { headExists = false } = {}) {
  const fsMod = fs;
  return (file, args) => {
    calls.push([file, ...args]);
    if (file === 'mvn' && args.includes('versions:set')) {
      // simulate what real `mvn versions:set -DnewVersion=X` does to the pom
      const pomPath = args[args.indexOf('-f') + 1];
      const newVersion = args.find((a) => a.startsWith('-DnewVersion=')).split('=')[1];
      const pom = fsMod.readFileSync(pomPath, 'utf-8');
      fsMod.writeFileSync(pomPath, pom.replace(/<version>[^<]*<\/version>/, `<version>${newVersion}</version>`), 'utf-8');
      return { status: 0 };
    }
    if (file === 'curl') {
      const isUpload = args.includes('-T');
      if (!isUpload && headExists) return { status: 0 };
      return { status: isUpload ? 0 : 1 };
    }
    return { status: 0 };
  };
}

test('validateMavenVersion rejects -SNAPSHOT for release (exit 2)', () => {
  assert.throws(
    () => validateMavenVersion({ version: '0.4.1-SNAPSHOT', kind: 'release' }),
    (err) => {
      assert.ok(err instanceof UsageError);
      assert.equal(err.exitCode, 2);
      assert.equal(err.code, 'MAVEN_VERSION_SNAPSHOT');
      return true;
    },
  );
});

test('validateMavenVersion rejects non-SNAPSHOT for snapshot and accepts a proper one', () => {
  assert.throws(
    () => validateMavenVersion({ version: '0.4.0', kind: 'snapshot' }),
    (err) => {
      assert.ok(err instanceof CoderError);
      assert.equal(err.exitCode, 2);
      assert.equal(err.code, 'MAVEN_VERSION_NOT_SNAPSHOT');
      return true;
    },
  );
  assert.doesNotThrow(() => validateMavenVersion({ version: '0.4.1-SNAPSHOT', kind: 'snapshot' }));
});

test('credential priority: CLI beats env', () => {
  const creds = resolveCredentials({
    args: { user: 'a', pass: 'secret' },
    env: { MAVEN_USERNAME: 'b', MAVEN_PASSWORD: 'p' },
    wizard: null,
    repoId: 'reposilite-repository-releases',
    settingsXml: null,
  });
  assert.equal(creds.user, 'a');
  assert.match(creds.source, /CLI/);
});

test('credential priority: env beats wizard config', () => {
  const creds = resolveCredentials({
    args: {},
    env: { MAVEN_USERNAME: 'b', MAVEN_PASSWORD: 'p' },
    wizard: { mavenAuthMode: 'userpass', mavenUsername: 'w', mavenPassword: 'pw' },
    repoId: 'x',
    settingsXml: null,
  });
  assert.equal(creds.user, 'b');
});

test('deriveRepoUrl appends releases/snapshots per variant', () => {
  assert.equal(deriveRepoUrl('http://host:18181', 'release'), 'http://host:18181/releases');
  assert.equal(deriveRepoUrl('http://host:18181/', 'snapshot'), 'http://host:18181/snapshots');
});

test('publishMaven curl mode: versions:set first, install before uploads, count matches', () => {
  const dir = fixture({ version: '1.7.0', artifacts: 2 });
  const calls = [];
  const result = publishMaven({
    root: dir,
    version: '1.7.0',
    kind: 'release',
    exec: fakeExec(calls),
    env: { MAVEN_USERNAME: 'u', MAVEN_PASSWORD: 'p' },
    m2RepoRoot: path.join(dir, 'm2'),
    settingsPath: path.join(dir, 'no-settings.xml'),
    out: { log() {}, warn() {} },
  });
  assert.equal(result.modules, 4, '2 artifacts × (pom+jar)');
  assert.equal(result.uploaded, 4);
  const setIdx = calls.findIndex((c) => c[0] === 'mvn' && c.includes('versions:set'));
  const installIdx = calls.findIndex((c) => c[0] === 'mvn' && c.includes('install'));
  const firstUploadIdx = calls.findIndex((c) => c[0] === 'curl' && c.includes('-T'));
  assert.ok(setIdx >= 0, 'versions:set runs');
  assert.ok(installIdx > setIdx, 'install runs after versions:set');
  assert.ok(firstUploadIdx > installIdx, 'uploads run after install');
});
test('publishMaven dry-run runs verify only and uploads nothing', () => {
  const dir = fixture();
  const calls = [];
  const result = publishMaven({
    root: dir,
    version: '1.7.0',
    kind: 'release',
    dryRun: true,
    exec: fakeExec(calls),
    env: {},
    m2RepoRoot: path.join(dir, 'm2'),
    settingsPath: path.join(dir, 'no-settings.xml'),
    out: { log() {}, warn() {} },
  });
  assert.equal(result.dryRun, true);
  assert.equal(result.uploaded, 0);
  assert.equal(calls.filter((c) => c[0] === 'curl' && c.includes('-T')).length, 0, 'no uploads on dry-run');
  const mvn = calls.filter((c) => c[0] === 'mvn' && !c.includes('--version'));
  assert.equal(mvn.length, 1, 'single mvn clean verify');
  assert.ok(mvn[0].includes('verify'));
});
