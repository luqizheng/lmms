'use strict';

/**
 * lib/check.js — `cms-sync --check` mode: print current sync state.
 */

const fs = require('node:fs');
const path = require('node:path');
const r = require('./report.js');

function runCheck(opts) {
  const root = path.resolve(opts.root);
  const policyPath = opts.policy || path.join(root, '.upstream-sync.yaml');
  const lockPath = opts.lock || path.join(root, '.upstream.lock');

  let exitCode = 0;

  if (fs.existsSync(lockPath)) {
    r.section('current sync state');
    const data = JSON.parse(fs.readFileSync(lockPath, 'utf8'));
    for (const [k, v] of Object.entries(data)) {
      console.log(`  ${k}: ${v}`);
    }
  } else {
    r.warn('no .upstream.lock found — never synced');
    exitCode = 1;
  }

  if (fs.existsSync(policyPath)) {
    r.section('policy file');
    console.log(`  ${policyPath}`);
  } else {
    r.err(`no policy file found at ${policyPath}`);
    exitCode = 1;
  }

  return exitCode;
}

module.exports = { runCheck };