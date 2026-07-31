'use strict';

/**
 * lib/manifest.js — parse MANIFEST.json (new release) and .upstream.lock (previous).
 */

const fs = require('node:fs');

function readJsonSafe(path, fallback) {
  try {
    return JSON.parse(fs.readFileSync(path, 'utf8'));
  } catch {
    return fallback;
  }
}

/**
 * Read the new release manifest. Throws if file is missing — that's a hard
 * error because every dist MUST include a MANIFEST.json (release.sh enforces).
 */
function loadNewManifest(distRoot) {
  const path = require('node:path').join(distRoot, 'MANIFEST.json');
  if (!fs.existsSync(path)) {
    throw new Error(`MANIFEST.json not found in dist root: ${distRoot}`);
  }
  const data = JSON.parse(fs.readFileSync(path, 'utf8'));
  for (const k of ['version', 'git_sha']) {
    if (!data[k]) {
      throw new Error(`MANIFEST.json missing required field: ${k}`);
    }
  }
  return data;
}

/**
 * Read the previous sync lock. Returns null if not present — that's the
 * first-sync case where 3-way merge falls back to overwrite.
 */
function loadPreviousLock(lockPath) {
  if (!fs.existsSync(lockPath)) return null;
  return readJsonSafe(lockPath, null);
}

module.exports = { loadNewManifest, loadPreviousLock };