'use strict';

// Version ledger reconciliation. Authority = git tag (v-prefixed); the other
// two ledgers (dist-template Makefile IMAGE_TAG, dist CHANGELOG.md) are
// compared against it. All upstream functions are injectable for tests.
//
// drift semantics: empty when everything is null (fresh repo) or everything
// matches; otherwise one entry per ledger that is missing or disagrees,
// 'git' included when no tag baseline exists at all.

const fs = require('node:fs');
const path = require('node:path');
const repo = require('./repo.js');
const { latestTag } = require('./version.js');
const { readImageTag } = require('./makefile.js');
const { readTopVersion } = require('./changelog.js');

function readOrNull(filePath) {
  try {
    return fs.readFileSync(filePath, 'utf-8');
  } catch {
    return null;
  }
}

function collectStatus({ root, git = repo.git, dirty = repo.dirtyCount, branch = repo.currentBranch } = {}) {
  let tagOutput = '';
  let gitFailed = false;
  try {
    tagOutput = git(root, ['tag', '-l']);
  } catch {
    gitFailed = true;
  }
  const version = gitFailed ? null : latestTag(tagOutput);
  const baseline = version ? version.image : null;

  const makefilePath = root ? path.join(root, 'release', 'dist-template', 'Makefile') : null;
  const changelogPath = root ? path.join(root, 'release', 'dist-template', 'CHANGELOG.md') : null;
  const makefileText = makefilePath ? readOrNull(makefilePath) : null;
  const changelogText = changelogPath ? readOrNull(changelogPath) : null;
  const makefileTag = makefileText == null ? null : readImageTag(makefileText);
  const changelogVersion = changelogText == null ? null : readTopVersion(changelogText);

  const allNull = baseline == null && makefileTag == null && changelogVersion == null;
  const drift = [];
  if (!allNull) {
    if (baseline == null) drift.push('git');
    if (makefileTag != null && (baseline == null || makefileTag !== baseline)) drift.push('makefile');
    if (changelogVersion != null && (baseline == null || changelogVersion !== baseline)) drift.push('changelog');
  }

  return {
    git: {
      latestTag: version ? version.tag : null,
      imageTag: baseline,
      dirty: root ? dirty(root) : -1,
      branch: root ? branch(root) : '(unknown)',
    },
    makefile: { path: makefilePath, imageTag: makefileTag },
    changelog: { path: changelogPath, version: changelogVersion },
    consistent: allNull || drift.length === 0,
    drift,
  };
}

module.exports = { collectStatus };
