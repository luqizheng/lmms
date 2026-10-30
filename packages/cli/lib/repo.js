'use strict';

const { execFileSync } = require('node:child_process');
const { ExternalError } = require('./errors.js');
const { detectContext } = require('./context.js');

function findRoot(cwd) {
  return detectContext(cwd).root;
}

function git(root, args) {
  try {
    return execFileSync('git', args, { cwd: root, encoding: 'utf-8' }).trim();
  } catch (err) {
    const firstLine = String((err.stderr && err.stderr.trim()) || err.message).split('\n')[0].trim();
    throw new ExternalError(`git ${args.join(' ')} failed: ${firstLine}`, { args });
  }
}

function dirtyCount(root) {
  try {
    const out = execFileSync('git', ['status', '--porcelain'], { cwd: root, encoding: 'utf-8' });
    return out ? out.split('\n').filter(Boolean).length : 0;
  } catch {
    return -1;
  }
}

function currentBranch(root) {
  try {
    return execFileSync('git', ['rev-parse', '--abbrev-ref', 'HEAD'], { cwd: root, encoding: 'utf-8' }).trim();
  } catch {
    return '(unknown)';
  }
}

module.exports = { findRoot, git, dirtyCount, currentBranch };
