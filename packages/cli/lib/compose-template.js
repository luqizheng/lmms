'use strict';

// Publish the infrastructure/ compose layout as a versioned base template.
// Downstream repos (ai-pm) copy the bundle and adapt it; same compose project
// name ("infrastructure" dir basename) auto-joins the superpowers network.

const fs = require('node:fs');
const path = require('node:path');

// Top-level names never shipped in a template bundle. README.md is lmms-owned
// documentation — consuming repos write their own (ruling 2026-10-30: template
// docs belong to the consuming project).
const EXCLUDE_NAMES = new Set(['AGENTS.md', 'STATE.md', 'LOOP.md', 'README.md', 'vendor']);
// Session/loop scratch notes at any depth.
const EXCLUDE_PATTERNS = [/^loop-.*\.md$/];

function isExcluded(relSegments, name) {
  if (EXCLUDE_PATTERNS.some((re) => re.test(name))) return true;
  if (relSegments.length === 0 && EXCLUDE_NAMES.has(name)) return true;
  // dotfiles/dirs are skipped, except the whitelisted env example
  if (name.startsWith('.') && name !== '.env.prod.example') return true;
  return false;
}

// Walk infraDir breadth-first; return [{ rel, abs }] with forward-slash rel.
function collectTemplateFiles(infraDir, fsMod = fs) {
  const out = [];
  const walk = (dir, segments) => {
    for (const entry of fsMod.readdirSync(dir, { withFileTypes: true })) {
      if (isExcluded(segments, entry.name)) continue;
      const abs = path.join(dir, entry.name);
      const rel = [...segments, entry.name].join('/');
      if (entry.isDirectory()) {
        walk(abs, [...segments, entry.name]);
      } else if (entry.isFile()) {
        out.push({ rel, abs });
      }
    }
  };
  walk(infraDir, []);
  return out.sort((a, b) => a.rel.localeCompare(b.rel));
}

// Build <outRoot>/coder-compose-template-<version>/ from infraDir.
// Overwrites a previous bundle of the same version. Returns { dir, fileCount }.
function buildComposeTemplate({ infraDir, outRoot, version, fsMod = fs }) {
  const files = collectTemplateFiles(infraDir, fsMod);
  const dir = path.join(outRoot, `coder-compose-template-${version}`);
  fsMod.rmSync(dir, { recursive: true, force: true });
  fsMod.mkdirSync(dir, { recursive: true });

  for (const { rel, abs } of files) {
    const dest = path.join(dir, ...rel.split('/'));
    fsMod.mkdirSync(path.dirname(dest), { recursive: true });
    fsMod.copyFileSync(abs, dest);
  }

  const info = {
    name: 'coder-compose-template',
    version,
    generatedAt: new Date().toISOString(),
    fileCount: files.length,
    source: 'low-memory-micro-services/infrastructure',
  };
  fsMod.writeFileSync(path.join(dir, 'TEMPLATE_INFO.json'), JSON.stringify(info, null, 2) + '\n', 'utf-8');
  return { dir, fileCount: files.length };
}

module.exports = { collectTemplateFiles, buildComposeTemplate };
