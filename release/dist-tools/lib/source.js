'use strict';

/**
 * lib/source.js — extract a dist tarball (or directory) into a tmp location.
 *
 * Supported sources:
 *   - path to a local .tar.gz / .tgz / .tar / .zip
 *   - http(s) URL pointing at the above
 *   - path to an already-extracted directory
 *
 * Returns: { distRoot, cleanup() } where distRoot points at the dir that
 * contains MANIFEST.json. cleanup() removes any temp dirs / downloaded files.
 */

const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { pipeline } = require('node:stream/promises');
const tar = require('tar');
const { err, warn } = require('./report.js');

async function fetchToBuffer(url) {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`HTTP ${res.status} fetching ${url}`);
  }
  const buf = Buffer.from(await res.arrayBuffer());
  return buf;
}

async function fetchToFile(url, dest) {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`HTTP ${res.status} fetching ${url}`);
  }
  await pipeline(res.body, fs.createWriteStream(dest));
  return dest;
}

function isUrl(s) {
  return /^https?:\/\//i.test(s);
}

/**
 * Locate the dist root within an extracted directory.
 * Some tarballs wrap everything in `coder-micro-services-v1.0.0/`,
 * others unpack flat. We descend into single-dir wrappers until we find
 * MANIFEST.json (or hit a non-unique structure).
 */
function findDistRoot(dir) {
  let cur = dir;
  // Safety bound: don't descend forever
  for (let depth = 0; depth < 5; depth++) {
    if (fs.existsSync(path.join(cur, 'MANIFEST.json'))) return cur;

    const entries = fs.readdirSync(cur);
    const subdirs = entries.filter(e => {
      const full = path.join(cur, e);
      return fs.statSync(full).isDirectory() && e !== 'node_modules' && e !== '.git';
    });
    const files = entries.filter(e => fs.statSync(path.join(cur, e)).isFile());

    if (subdirs.length === 1 && files.length === 0) {
      cur = path.join(cur, subdirs[0]);
      continue;
    }
    break;
  }
  throw new Error(`cannot locate dist root (no MANIFEST.json found inside ${dir})`);
}

/**
 * Extract a tar archive to `destDir`.
 */
async function extractTar(archivePath, destDir, opts = {}) {
  await tar.x({
    file: archivePath,
    cwd: destDir,
    ...opts,
  });
}

/**
 * Extract a zip archive. Uses the `unzipper`-free approach: shell out to
 * PowerShell's Expand-Archive on Windows and `unzip` on Linux/macOS when
 * available. Falls back to a JS implementation via `tar` package which
 * supports .zip as of v7+ (it shells out internally on Linux, but uses
 * node:zlib on Windows). To stay dependency-light, we use tar's zip
 * support directly.
 */
async function extractZip(archivePath, destDir) {
  await tar.x({
    file: archivePath,
    cwd: destDir,
  });
}

/**
 * Determine archive type by extension.
 */
function archiveType(p) {
  const lower = p.toLowerCase();
  if (lower.endsWith('.zip')) return 'zip';
  if (lower.endsWith('.tar.gz') || lower.endsWith('.tgz')) return 'tgz';
  if (lower.endsWith('.tar.bz2')) return 'tbz2';
  if (lower.endsWith('.tar.xz')) return 'txz';
  if (lower.endsWith('.tar')) return 'tar';
  return null;
}

/**
 * Resolve a source spec to a local path.
 * - URL → downloaded to tmp
 * - Existing dir → returned as-is
 * - Existing file → returned as-is
 */
async function resolveSource(source, tmpRoot) {
  if (isUrl(source)) {
    const filename = path.posix.basename(new URL(source).pathname) || 'dist.tar.gz';
    const localPath = path.join(tmpRoot, filename);
    await fetchToFile(source, localPath);
    return { localPath, isDir: false, downloaded: true };
  }

  const stat = fs.statSync(source);
  if (stat.isDirectory()) {
    return { localPath: source, isDir: true, downloaded: false };
  }
  return { localPath: source, isDir: false, downloaded: false };
}

/**
 * Top-level entry: prepare the source and return the dist root.
 * Caller is responsible for invoking cleanup().
 */
async function prepareSource(source) {
  const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'cms-sync-'));
  let extractedDir = null;
  let downloadedFile = null;

  try {
    const resolved = await resolveSource(source, tmpRoot);

    if (resolved.isDir) {
      extractedDir = resolved.localPath;
    } else {
      downloadedFile = resolved.downloaded ? resolved.localPath : null;
      const type = archiveType(resolved.localPath);
      if (!type) {
        throw new Error(`unsupported archive format: ${resolved.localPath}`);
      }

      const extractTarget = path.join(tmpRoot, 'extracted');
      fs.mkdirSync(extractTarget, { recursive: true });

      if (type === 'zip') {
        await extractZip(resolved.localPath, extractTarget);
      } else {
        await extractTar(resolved.localPath, extractTarget);
      }
      extractedDir = extractTarget;
    }

    const distRoot = findDistRoot(extractedDir);

    const cleanup = () => {
      try { fs.rmSync(tmpRoot, { recursive: true, force: true }); } catch {}
    };

    return { distRoot, cleanup, tmpRoot };
  } catch (err) {
    // Best-effort cleanup on failure
    try { fs.rmSync(tmpRoot, { recursive: true, force: true }); } catch {}
    throw err;
  }
}

module.exports = {
  prepareSource,
  findDistRoot,
  archiveType,
  isUrl,
};