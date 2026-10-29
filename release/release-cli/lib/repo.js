'use strict';

/**
 * lib/repo.js — repo root discovery and the few git calls the CLI needs.
 * All sync and tiny on purpose.
 */

const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

// Walk up from startDir looking for the repo marker (scripts/build-docker.js).
function findRoot(startDir) {
  let dir = path.resolve(startDir);
  for (;;) {
    if (fs.existsSync(path.join(dir, 'scripts', 'build-docker.js'))) return dir;
    const parent = path.dirname(dir);
    if (parent === dir) return null;
    dir = parent;
  }
}

function git(root, args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf-8' }).trim();
}

// Latest v-semver tag as a normalized version; null when git is unavailable
// or the repo has no v* tags.
function latestVersion(root) {
  try {
    const { latestTag } = require('./version');
    return latestTag(git(root, ['tag', '-l']));
  } catch {
    return null;
  }
}

// Number of uncommitted changes; -1 when git is unavailable.
function dirtyCount(root) {
  try {
    const out = git(root, ['status', '--porcelain']);
    return out ? out.split('\n').filter(Boolean).length : 0;
  } catch {
    return -1;
  }
}

function currentBranch(root) {
  try {
    return git(root, ['rev-parse', '--abbrev-ref', 'HEAD']);
  } catch {
    return '(unknown)';
  }
}

module.exports = { findRoot, git, latestVersion, dirtyCount, currentBranch };
