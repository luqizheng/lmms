'use strict';

// Release target resolution and ledger rewrite. Safety rails (spec 六·补 #2):
// git tag is authoritative; bump refuses on ledger drift unless --force
// --from-tag; nothing here ever pushes.

const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { UsageError, GuardError } = require('./errors.js');
const { normalizeVersion, bumpVersion } = require('./version.js');
const { applyVersion } = require('./makefile.js');
const { insertEntry, hasVersion } = require('./changelog.js');

function resolveTarget({ level = 'minor', version, force = false, fromTag = false, status } = {}) {
  if (!status) throw new UsageError('resolveTarget requires a release status');
  if (!status.consistent && !(force && fromTag)) {
    throw new GuardError('VERSION_DRIFT', 'version ledger drifted from git tag — converge with --force --from-tag or fix by hand', { drift: status.drift });
  }

  if (version != null) {
    const parsed = normalizeVersion(version);
    if (!parsed) throw new UsageError(`invalid version: ${version}`);
    return { version: parsed.image, imageTag: parsed.image, converge: false };
  }

  const baseline = normalizeVersion(status.git.latestTag);
  if (!baseline) {
    throw new GuardError('NO_BASELINE', 'no v* tag found to establish a baseline', { latestTag: status.git.latestTag });
  }
  if (force && fromTag) {
    return { version: baseline.image, imageTag: baseline.image, converge: true };
  }
  const bumped = bumpVersion(baseline, level);
  return { version: bumped.image, imageTag: bumped.image, converge: false };
}

function bumpLedger({ root, imageTag, changelogBody, date, fs: nodeFs = fs } = {}) {
  const distDir = path.join(root, 'release', 'dist-template');
  const makefilePath = path.join(distDir, 'Makefile');
  const changelogPath = path.join(distDir, 'CHANGELOG.md');
  const changed = [];

  const mkBefore = nodeFs.readFileSync(makefilePath, 'utf-8');
  const mkAfter = applyVersion(mkBefore, imageTag);
  if (mkAfter !== mkBefore) {
    nodeFs.writeFileSync(makefilePath, mkAfter, 'utf-8');
    changed.push('release/dist-template/Makefile');
  }

  const clBefore = nodeFs.readFileSync(changelogPath, 'utf-8');
  let clAfter = clBefore;
  if (!hasVersion(clBefore, imageTag)) {
    const today = date ?? new Date().toISOString().slice(0, 10);
    clAfter = insertEntry(clBefore, { version: imageTag, date: today, body: changelogBody ?? `### Maintenance\n- version ledger converged to v${imageTag}` });
  }
  if (clAfter !== clBefore) {
    nodeFs.writeFileSync(changelogPath, clAfter, 'utf-8');
    changed.push('release/dist-template/CHANGELOG.md');
  }

  return { changed };
}

function commitLedger({ root, imageTag, exec = execFileSync } = {}) {
  const message = `chore(release): bump version ledger to ${imageTag}`;
  exec('git', ['add', 'release/dist-template/Makefile', 'release/dist-template/CHANGELOG.md'], { cwd: root, encoding: 'utf-8' });
  exec('git', ['commit', '-m', message], { cwd: root, encoding: 'utf-8' });
  return { commit: message };
}

module.exports = { resolveTarget, bumpLedger, commitLedger };
