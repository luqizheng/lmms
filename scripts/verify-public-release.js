'use strict';

// Verify that every versioned artifact is actually readable from its public registry.
// This script is intentionally read-only and never prints credentials.

const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const sdkPublish = require('../packages/cli/lib/sdk-publish');
const imageEnv = require('../packages/cli/lib/image-env');
const { readServiceVersions } = require('../packages/cli/lib/service-versions');

const root = path.resolve(__dirname, '..');
const ecosystem = process.argv[2];

async function verifyHttp(url, label) {
  const response = await fetch(url, {
    headers: { 'user-agent': 'lmms-public-release-verifier/1.0' },
  });
  if (!response.ok) {
    throw new Error(`${label}: registry returned HTTP ${response.status}`);
  }
  return response;
}

async function verifyRust() {
  const crates = [];
  for (const dir of sdkPublish.LIBS_ORDER) {
    crates.push({ base: 'libs', dir });
  }
  for (const dir of sdkPublish.SDKS_ORDER) {
    crates.push({ base: path.join('sdk', 'rust'), dir });
  }
  for (const item of crates) {
    const pkg = sdkPublish.readCrateManifest(root, item.base, item.dir);
    const url = `https://crates.io/api/v1/crates/${encodeURIComponent(pkg.name)}/${encodeURIComponent(pkg.version)}`;
    await verifyHttp(url, `crates.io ${pkg.name}@${pkg.version}`);
    console.log(`verified crates.io ${pkg.name}@${pkg.version}`);
  }
  console.log(`Verified ${crates.length} Rust crates on crates.io.`);
}

async function verifyNpm() {
  for (const item of sdkPublish.NPM_PACKAGES) {
    const manifest = JSON.parse(fs.readFileSync(path.join(root, item.dir, 'package.json'), 'utf8'));
    if (manifest.private === true) continue;
    const encodedName = encodeURIComponent(manifest.name || item.name);
    const url = `https://registry.npmjs.org/${encodedName}/${encodeURIComponent(manifest.version)}`;
    await verifyHttp(url, `npm ${manifest.name || item.name}@${manifest.version}`);
    console.log(`verified npm ${manifest.name || item.name}@${manifest.version}`);
  }
  console.log('Verified all configured npm packages on registry.npmjs.org.');
}

function verifyDocker() {
  const namespace = process.env.DOCKER_USER_NAME;
  if (!namespace || !/^[a-z0-9][a-z0-9_.-]*$/i.test(namespace)) {
    throw new Error('DOCKER_USER_NAME must be a valid Docker Hub namespace.');
  }
  const versionInfo = readServiceVersions(root);
  if (!versionInfo.version || versionInfo.splits.length || versionInfo.missing.length) {
    throw new Error('Service image versions are missing or inconsistent.');
  }
  const refs = imageEnv.imageRefs({
    version: versionInfo.version,
    dialect: 'pg',
    registry: 'docker.io',
    repo: namespace,
  });
  for (const image of refs) {
    execFileSync('docker', ['buildx', 'imagetools', 'inspect', image.ref], { stdio: 'ignore' });
    console.log(`verified Docker Hub ${image.ref}`);
  }
  console.log(`Verified ${refs.length} Docker Hub images.`);
}

(async () => {
  if (ecosystem === 'rust') await verifyRust();
  else if (ecosystem === 'npm') await verifyNpm();
  else if (ecosystem === 'docker') verifyDocker();
  else throw new Error('Usage: node scripts/verify-public-release.js <rust|npm|docker>');
})().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
