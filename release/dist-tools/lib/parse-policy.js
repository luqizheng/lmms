#!/usr/bin/env node
/**
 * parse-policy.js — Parse .upstream-sync.yaml into structured JSON.
 *
 * The policy format is intentionally simple (no nested mappings, no anchors)
 * so we can avoid pulling in js-yaml as a runtime dependency. If your policy
 * needs to grow, switch this to js-yaml.
 *
 * Output shape:
 *   { managed: [...], merge: [...], skip: [...] }
 */
'use strict';

const fs = require('node:fs');
const path = require('node:path');

function parseArgs(argv) {
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith('--')) continue;
    if (a.includes('=')) {
      const [k, v] = a.slice(2).split(/=(.+)/);
      opts[k] = v;
    } else {
      opts[a.slice(2)] = argv[++i];
    }
  }
  return opts;
}

/**
 * Tiny YAML-ish parser for our limited policy format.
 * Recognizes:
 *   - top-level keys with list values, optionally with `[]` shorthand for empty
 *   - list items prefixed with `- ` or `  - `
 *   - comment lines starting with #
 *   - blank lines
 */
function parsePolicy(text) {
  const lines = text.split(/\r?\n/);
  const result = {};
  let currentKey = null;

  for (let i = 0; i < lines.length; i++) {
    const raw = lines[i];
    const line = raw.replace(/#.*$/, '').replace(/\s+$/, '');
    if (!line.trim()) continue;

    const keyMatch = line.match(/^([a-zA-Z_][\w-]*)\s*:\s*(\[\])?\s*$/);
    if (keyMatch) {
      currentKey = keyMatch[1];
      if (!result[currentKey]) result[currentKey] = [];
      continue;
    }

    const itemMatch = line.match(/^\s*-\s+(.+)$/);
    if (itemMatch && currentKey) {
      const value = itemMatch[1].trim().replace(/^["']|["']$/g, '');
      if (value) result[currentKey].push(value);
      continue;
    }

    if (line.trim()) {
      console.error(`[parse-policy] ⚠ line ${i + 1} ignored: ${raw}`);
    }
  }

  return {
    managed: result.managed || [],
    merge: result.merge || result['three-way-merge'] || [],
    skip: result.skip || [],
    'protected-managed': result['protected-managed'] || [],
  };
}

/**
 * Load and parse a policy file from disk. Throws on missing file.
 */
function loadPolicy(policyPath) {
  if (!fs.existsSync(policyPath)) {
    throw new Error(`policy file not found: ${policyPath}`);
  }
  const text = fs.readFileSync(policyPath, 'utf8');
  return parsePolicy(text);
}

function main() {
  const opts = parseArgs(process.argv.slice(2));
  if (!opts.policy) {
    console.error('usage: parse-policy.js --policy <yaml> [--out <json>]');
    process.exit(2);
  }
  const parsed = loadPolicy(opts.policy);

  if (opts.out) {
    fs.mkdirSync(path.dirname(opts.out), { recursive: true });
    fs.writeFileSync(opts.out, JSON.stringify(parsed, null, 2) + '\n');
  } else {
    process.stdout.write(JSON.stringify(parsed, null, 2) + '\n');
  }
}

if (require.main === module) main();

module.exports = { parsePolicy, loadPolicy };