'use strict';

const fs = require('node:fs');
const path = require('node:path');

const FROM_REGEX = /^FROM\s+(\S+)/gm;
const COMPOSE_IMAGE_REGEX = /^\s*image:\s*(\S+)/gm;

function parseDockerfile(content) {
  const refs = [];
  const seen = new Set();
  for (const m of content.matchAll(FROM_REGEX)) {
    const ref = m[1];
    if (ref.startsWith('--')) continue;
    const stage = ref.split(/\s+/)[0];
    if (!seen.has(stage)) {
      seen.add(stage);
      refs.push(stage);
    }
  }
  return refs;
}

function parseCompose(content) {
  const refs = [];
  const seen = new Set();
  for (const m of content.matchAll(COMPOSE_IMAGE_REGEX)) {
    const ref = m[1].replace(/['"]/g, '');
    if (seen.has(ref)) continue;
    seen.add(ref);
    refs.push(ref);
  }
  return refs;
}

function collectFiles(root, rel) {
  const abs = path.join(root, rel);
  if (!fs.existsSync(abs)) return [];
  const stat = fs.statSync(abs);
  if (stat.isFile()) return [abs];
  const out = [];
  for (const e of fs.readdirSync(abs)) {
    if (e === 'Dockerfile' || e === 'Dockerfile.local' || e.startsWith('docker-compose')) {
      out.push(path.join(abs, e));
    }
  }
  return out;
}

function scanProject(root) {
  const dockerfiles = collectFiles(root, 'services');
  const composeFiles = collectFiles(root, 'infrastructure');
  const seen = new Map();
  for (const f of dockerfiles) {
    const content = fs.readFileSync(f, 'utf8');
    for (const ref of parseDockerfile(content)) {
      if (seen.has(ref)) continue;
      const rel = path.relative(root, f);
      seen.set(ref, { ref, source: rel, mirror: null });
    }
  }
  for (const f of composeFiles) {
    const content = fs.readFileSync(f, 'utf8');
    for (const ref of parseCompose(content)) {
      if (seen.has(ref)) continue;
      const rel = path.relative(root, f);
      seen.set(ref, { ref, source: rel, mirror: null });
    }
  }
  return Array.from(seen.values());
}

module.exports = { parseDockerfile, parseCompose, scanProject };