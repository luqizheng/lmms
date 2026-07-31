'use strict';

/**
 * lib/expand-globs.js — expand glob patterns against a directory tree.
 *
 * Public API: `expandGlobs(patterns, distRoot)` returns a sorted, deduplicated
 * array of relative file paths matched by any of the patterns.
 *
 * Patterns supported:
 *   **  matches any number of path segments (including /)
 *   *   matches any number of non-separator chars
 *   ?   matches any single non-separator char
 *
 * Skips node_modules and .git automatically.
 */

const fs = require('node:fs');
const path = require('node:path');

function globToRegex(pattern) {
  let out = '^';
  let i = 0;
  while (i < pattern.length) {
    const c = pattern[i];
    if (c === '*' && pattern[i + 1] === '*') {
      out += '.*';
      i += 2;
      if (pattern[i] === '/') i++;
    } else if (c === '*') {
      out += '[^/]*';
      i++;
    } else if (c === '?') {
      out += '[^/]';
      i++;
    } else if ('.+^${}()|[]\\'.includes(c)) {
      out += '\\' + c;
      i++;
    } else {
      out += c;
      i++;
    }
  }
  out += '$';
  return new RegExp(out);
}

function walk(dir, base, out) {
  if (!fs.existsSync(dir)) return;
  const st = fs.statSync(dir);
  if (st.isFile()) { out.push(base); return; }
  for (const e of fs.readdirSync(dir)) {
    if (e === 'node_modules' || e === '.git') continue;
    if (e.endsWith('.merge-base')) continue;
    const full = path.join(dir, e);
    const rel = base ? base + '/' + e : e;
    if (fs.statSync(full).isDirectory()) walk(full, rel, out);
    else out.push(rel);
  }
}

// Always use POSIX-style separators in patterns and results so that
// `foo/**` works the same on Windows as on Linux.  Patterns documented
// in user-facing help are POSIX.
function toPosix(p) {
  return p.split(path.sep).join('/');
}

/**
 * @param {string[]} patterns - glob patterns
 * @param {string}   distRoot - directory to walk
 * @returns {string[]} sorted, deduped relative paths
 */
function expandGlobs(patterns, distRoot) {
  if (!patterns || patterns.length === 0) return [];

  const files = [];
  walk(distRoot, '', files);

  const matched = new Set();
  for (const pat of patterns) {
    const re = globToRegex(pat);
    for (const f of files) {
      if (re.test(f)) matched.add(toPosix(f));
    }
  }
  return [...matched].sort();
}

if (require.main === module) {
  const [, , modeArg, policyArg, rootArg] = process.argv;
  if (!modeArg || !policyArg || !rootArg) {
    console.error('usage: expand-globs.js <mode> <policy.json> <dist-root>');
    process.exit(2);
  }
  const policy = JSON.parse(fs.readFileSync(policyArg, 'utf8'));
  const patterns = policy[modeArg] || [];
  const result = expandGlobs(patterns, rootArg);
  process.stdout.write(result.join('\n') + '\n');
}

module.exports = { expandGlobs, globToRegex };