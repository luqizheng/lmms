'use strict';

/**
 * lib/sync.js — orchestrate the full sync flow.
 *
 * Steps:
 *   1. Resolve source → dist root
 *   2. Load new MANIFEST.json
 *   3. Load previous .upstream.lock (if any)
 *   4. Load and parse .upstream-sync.yaml
 *   5. Expand glob patterns against the dist tree (managed, merge, skip)
 *   6. Apply skip exclusions to merge files; managed always wins over skip
 *      (explicit listing > wildcard). Warn on managed∖skip overlaps.
 *   7. For each managed file: overwrite
 *   8. For each three-way-merge file: 3-way merge (or overwrite if no base)
 *   9. Write updated .upstream.lock
 *  10. Print summary
 *
 * Returns the exit code to use.
 */

const fs = require('node:fs');
const path = require('node:path');

const { prepareSource } = require('./source.js');
const { loadNewManifest, loadPreviousLock } = require('./manifest.js');
const { loadPolicy } = require('./parse-policy.js');
const { expandGlobs } = require('./expand-globs.js');
const { isGitRepo, readFileAtRevision, threeWayMerge } = require('./git.js');
const r = require('./report.js');
const { EXIT_OK, EXIT_CONFLICT } = require('./cli.js');

async function runSync(opts) {
  const root = path.resolve(opts.root);
  const policyPath = opts.policy || path.join(root, '.upstream-sync.yaml');
  const lockPath = opts.lock || path.join(root, '.upstream.lock');

  if (!fs.existsSync(policyPath)) {
    throw new Error(`missing policy file: ${policyPath}\n` +
      `Create .upstream-sync.yaml in your project root first.`);
  }

  // 1. Resolve source
  r.log(`fetching source: ${opts.source}`);
  const { distRoot, cleanup } = await prepareSource(opts.source);

  try {
    // 2. Load new manifest
    const newManifest = loadNewManifest(distRoot);
    r.log(`new version: ${newManifest.version}  git_sha: ${newManifest.git_sha.slice(0, 7)}`);

    // 3. Load previous lock
    const prevLock = loadPreviousLock(lockPath);
    if (prevLock && prevLock.git_sha) {
      r.log(`previous version: ${prevLock.version}  git_sha: ${prevLock.git_sha.slice(0, 7)}`);
    } else {
      r.warn('no previous sync recorded — 3-way merge will fall back to no-base');
    }

    // 4. Parse policy
    const policy = loadPolicy(policyPath);

    // 5. Expand globs against dist tree
    const skipFiles = new Set(expandGlobs(policy.skip || [], distRoot));
    const rawManaged = expandGlobs(policy.managed, distRoot);
    const rawMerge = expandGlobs(policy.merge, distRoot);
    const rawProtected = expandGlobs(policy['protected-managed'] || [], distRoot);

    // 6. Apply skip: managed always wins over skip (explicit > wildcard).
    //    For merge files, skip wins (no accidental force-merge).
    //    protected-managed also wins over merge: those files would otherwise
    //    get overwritten by the merge loop *before* the sidecar block runs.
    const managedFiles = rawManaged;
    const protectedFiles = rawProtected.filter((f) => !skipFiles.has(f));
    const protectedSet = new Set(protectedFiles);
    const mergeFiles = rawMerge.filter((f) => !skipFiles.has(f) && !protectedSet.has(f));

    const overlaps = rawManaged.filter((f) => skipFiles.has(f));
    if (overlaps.length > 0) {
      r.warn(`${overlaps.length} file(s) listed in BOTH managed and skip — managed wins:`);
      for (const f of overlaps) r.log(`    ${f}`);
    }

    r.section(opts.dryRun ? 'DRY RUN — no files will be modified' : 'syncing files');

    // 7. Managed files: overwrite
    r.section('managed (auto-overwrite)');
    const counts = {
      managed: 0,
      merged: 0,
      conflicts: 0,
      added: 0,
      unchanged: 0,
      protectedCreated: 0,
      protectedSidecar: 0,
    };

    for (const rel of managedFiles) {
      const srcPath = path.join(distRoot, rel);
      const dstPath = path.join(root, rel);

      if (fileIdentical(srcPath, dstPath)) {
        counts.unchanged++;
        r.item('unchanged', 'dim', rel);
        continue;
      }

      if (!opts.dryRun) {
        fs.mkdirSync(path.dirname(dstPath), { recursive: true });
        fs.copyFileSync(srcPath, dstPath);
      }
      counts.managed++;
      r.item('updated', 'green', rel);
    }

    // 8. Three-way merge files
    r.section('three-way merge');
    let hasConflict = false;
    const inGit = await isGitRepo(root);
    const oldSha = (prevLock && inGit) ? prevLock.git_sha : null;

    for (const rel of mergeFiles) {
      const srcPath = path.join(distRoot, rel);
      const dstPath = path.join(root, rel);

      // Case 1: dst doesn't exist (new file in dist)
      if (!fs.existsSync(dstPath)) {
        if (!opts.dryRun) {
          fs.mkdirSync(path.dirname(dstPath), { recursive: true });
          fs.copyFileSync(srcPath, dstPath);
        }
        counts.added++;
        r.item('new file', 'green', rel);
        continue;
      }

      if (fileIdentical(srcPath, dstPath)) {
        counts.unchanged++;
        r.item('unchanged', 'dim', rel);
        continue;
      }

      // Try to find base in git history
      let baseContent = null;
      if (oldSha) {
        const buf = await readFileAtRevision(oldSha, rel, root);
        if (buf) baseContent = buf.toString('utf8');
      }

      if (baseContent !== null) {
        // Real 3-way merge
        const current = fs.readFileSync(dstPath, 'utf8');
        const other = fs.readFileSync(srcPath, 'utf8');

        const { merged, conflict } = await threeWayMerge(current, baseContent, other);
        if (!opts.dryRun) {
          // Save the merge base alongside for diff viewing
          try {
            fs.writeFileSync(dstPath + '.merge-base', baseContent);
          } catch {}
          fs.writeFileSync(dstPath, merged);
        }
        if (conflict) {
          hasConflict = true;
          counts.conflicts++;
          r.item('CONFLICT', 'red', `${rel} (conflict markers in file)`);
        } else {
          counts.merged++;
          r.item('merged', 'green', rel + (opts.dryRun ? ' (dry-run)' : ''));
        }
      } else {
        // No base — overwrite with warning
        if (!opts.dryRun) {
          fs.mkdirSync(path.dirname(dstPath), { recursive: true });
          fs.copyFileSync(srcPath, dstPath);
        }
        counts.merged++;
        r.item('updated', 'yellow', `${rel} (no merge base)`);
      }
    }

    // 9. Protected-managed: dst untouched, upstream lands as .new.<ext>
    //    so the developer diffs and merges manually.
    if (protectedFiles.length > 0) {
      r.section('protected-managed (sidecar .new.<ext> when dst exists)');
      for (const rel of protectedFiles) {
        const srcPath = path.join(distRoot, rel);
        const dstPath = path.join(root, rel);

        if (!fs.existsSync(srcPath)) {
          // Upstream no longer ships this file — leave dst alone, no sidecar.
          counts.unchanged++;
          r.item('upstream missing — dst preserved', 'dim', rel);
          continue;
        }

        if (!fs.existsSync(dstPath)) {
          // First install: copy as-is, no sidecar suffix.
          if (!opts.dryRun) {
            fs.mkdirSync(path.dirname(dstPath), { recursive: true });
            fs.copyFileSync(srcPath, dstPath);
          }
          counts.protectedCreated++;
          r.item('created', 'green', rel);
          continue;
        }

        if (fileIdentical(srcPath, dstPath)) {
          counts.unchanged++;
          r.item('unchanged', 'dim', rel);
          continue;
        }

        // dst exists and differs — write src as <file>.new.<ext>, keep dst.
        const newPath = sidecarPath(dstPath);
        if (!opts.dryRun) {
          fs.mkdirSync(path.dirname(newPath), { recursive: true });
          fs.copyFileSync(srcPath, newPath);
        }
        counts.protectedSidecar++;
        r.warn(`protected-managed: dst preserved, upstream written to ${path.relative(root, newPath)}`);
        r.item('sidecar', 'yellow', `${rel} → ${path.relative(root, newPath)}`);
      }
    }

    // 9. Summary
    r.section('summary');
    r.log(`  managed overwritten:        ${counts.managed}`);
    r.log(`  three-way merged:           ${counts.merged}`);
    r.log(`  new files added:            ${counts.added}`);
    r.log(`  protected-managed created:  ${counts.protectedCreated}`);
    r.log(`  protected-managed sidecar:  ${counts.protectedSidecar}`);
    if (counts.conflicts > 0) {
      r.err(`  conflicts (need fix):       ${counts.conflicts}`);
    }
    r.log(`  unchanged:                  ${counts.unchanged}`);

    // 9. Update lock (only on successful sync, not dry-run, not conflicts)
    if (hasConflict) {
      r.err('merge conflicts present — resolve and re-commit');
      if (!opts.dryRun) {
        r.warn('.upstream.lock NOT updated due to conflicts');
        return EXIT_CONFLICT;
      }
    }

    if (!opts.dryRun && !hasConflict) {
      const lockData = {
        version: newManifest.version,
        git_sha: newManifest.git_sha,
        synced_at: new Date().toISOString(),
      };
      fs.writeFileSync(lockPath, JSON.stringify(lockData, null, 2) + '\n');
      r.ok(`updated ${path.basename(lockPath)} → ${newManifest.version} (${newManifest.git_sha.slice(0, 7)})`);
    }

    r.log('');
    r.ok('sync complete');
    return EXIT_OK;
  } finally {
    cleanup();
  }
}

function fileIdentical(a, b) {
  if (!fs.existsSync(a) || !fs.existsSync(b)) return false;
  const ca = fs.readFileSync(a);
  const cb = fs.readFileSync(b);
  return ca.equals(cb);
}

// For protected-managed: dst stays untouched, src lands as <file>.new.<ext>
// so the developer diffs and merges manually. Preserves no extension for
// extensionless names like `Makefile` (-> `Makefile.new`).
function sidecarPath(dstPath) {
  const ext = path.extname(dstPath);
  const base = dstPath.slice(0, dstPath.length - ext.length);
  return base + '.new' + ext;
}

module.exports = { runSync };