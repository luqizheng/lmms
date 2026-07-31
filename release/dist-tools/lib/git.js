'use strict';

/**
 * lib/git.js — thin wrapper over the git CLI for operations we need.
 *
 * Why not a JS library like `simple-git`? Because we only need two ops,
 * spawning the git CLI directly is much lighter and works identically
 * on every platform where git itself works.
 */

const { spawn } = require('node:child_process');
const path = require('node:path');

/**
 * Detect whether the current working dir is inside a git repo.
 * `gitPath` may be null/undefined to use cwd.
 */
async function isGitRepo(cwd) {
  return new Promise((resolve) => {
    const proc = spawn('git', ['rev-parse', '--git-dir'], { cwd, stdio: 'pipe' });
    let stdout = '';
    proc.stdout.on('data', d => { stdout += d.toString(); });
    proc.on('close', code => resolve(code === 0 && stdout.trim().length > 0));
    proc.on('error', () => resolve(false));
  });
}

/**
 * Read a file as it existed at a specific git revision.
 * Returns null if the file didn't exist at that revision.
 */
async function readFileAtRevision(revision, filePath, cwd) {
  return new Promise((resolve) => {
    const proc = spawn('git', ['show', `${revision}:${filePath}`], { cwd, stdio: ['ignore', 'pipe', 'pipe'] });
    const chunks = [];
    proc.stdout.on('data', d => chunks.push(d));
    let stderr = '';
    proc.stderr.on('data', d => { stderr += d.toString(); });
    proc.on('close', code => {
      if (code !== 0) {
        resolve(null);
        return;
      }
      resolve(Buffer.concat(chunks));
    });
    proc.on('error', () => resolve(null));
  });
}

/**
 * Run git merge-file to perform a 3-way merge.
 *
 * Inputs (in-memory strings, not files on disk):
 *   current  — the downstream's current version
 *   base     — the common ancestor (from upstream's last sync)
 *   other    — the new upstream version
 *
 * Returns:
 *   { merged, conflict }
 *     merged:   the merged content (with conflict markers if conflict)
 *     conflict: true if there were conflict regions
 *
 * git merge-file exits with:
 *   0  = clean merge (no conflicts)
 *   >0 = conflicts present
 *
 * Strategy: write inputs to temp files (git merge-file needs file args),
 * run the command, read the result, delete temp files.
 */
async function threeWayMerge(current, base, other) {
  const fs = require('node:fs/promises');
  const os = require('node:os');
  const tmpRoot = await fs.mkdtemp(path.join(os.tmpdir(), 'cms-merge-'));
  const curPath = path.join(tmpRoot, 'current');
  const basePath = path.join(tmpRoot, 'base');
  const otherPath = path.join(tmpRoot, 'other');

  try {
    await fs.writeFile(curPath, current);
    await fs.writeFile(basePath, base);
    await fs.writeFile(otherPath, other);

    const result = await new Promise((resolve, reject) => {
      const proc = spawn('git', ['merge-file', '-L', 'current', '-L', 'base', '-L', 'other', curPath, basePath, otherPath], { stdio: 'pipe' });
      let stderr = '';
      proc.stderr.on('data', d => { stderr += d.toString(); });
      proc.on('error', reject);
      proc.on('close', async (code) => {
        // git merge-file modifies curPath in place; read it back
        try {
          const merged = await fs.readFile(curPath, 'utf8');
          // Also copy base back as the merge-base artifact (for diff display)
          await fs.copyFile(basePath, curPath + '.merge-base').catch(() => {});
          resolve({ merged, conflict: code > 0, stderr });
        } catch (e) {
          reject(e);
        }
      });
    });

    return result;
  } finally {
    try { await fs.rm(tmpRoot, { recursive: true, force: true }); } catch {}
  }
}

module.exports = {
  isGitRepo,
  readFileAtRevision,
  threeWayMerge,
};