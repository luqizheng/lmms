'use strict';
// Base-Images-Scanner.js
// 扫描 Dockerfile（含 services/ sdk/ frontend/）、docker-compose*.yml（infrastructure/）、
// 以及根目录 .gitlab-ci.yml 中的基础镜像引用；对 Dockerfile 的 ARG 与 gitlab-ci 的 variables
// 做变量替换，把 ${NAME} / $NAME 展开成真实 ref。

const fs = require('node:fs');
const path = require('node:path');

const FROM_REGEX = /^FROM\s+(\S+)/gm;
const ARG_REGEX = /^ARG\s+(\w+)\s*=\s*(\S+)/gm;
const IMAGE_LINE_REGEX = /^\s*image:\s*(\S+)/gm;

const CI_VAR_LINE_REGEX = /^\s{2,}(\w+):\s*(\S+)/;
const VARIABLES_BLOCK_START = /^variables:\s*$/;

const SKIP_DIRS = new Set(['target', 'node_modules', '.git', 'reports', '.codegraph']);

function parseDockerArgs(content) {
  const args = new Map();
  for (const m of content.matchAll(ARG_REGEX)) {
    const name = m[1];
    const value = m[2].replace(/^["']|["']$/g, '');
    if (!args.has(name)) args.set(name, value);
  }
  return args;
}

function parseGitlabVariables(content) {
  const lines = content.split('\n');
  const vars = new Map();
  let inVariables = false;
  let blockIndent = -1;
  for (const raw of lines) {
    const line = raw.replace(/\s+$/, '');
    if (!line.trim() || line.trim().startsWith('#')) continue;
    const indent = line.match(/^(\s*)/)[1].length;
    if (!inVariables) {
      if (VARIABLES_BLOCK_START.test(line)) {
        inVariables = true;
        blockIndent = indent;
      }
      continue;
    }
    if (indent <= blockIndent) {
      inVariables = false;
      continue;
    }
    const m = line.match(CI_VAR_LINE_REGEX);
    if (m) {
      vars.set(m[1], m[2].replace(/^["']|["']$/g, ''));
    }
  }
  return vars;
}

function substituteVars(ref, vars) {
  return ref
    .replace(/\$\{(\w+)\}/g, (_, name) => (vars.has(name) ? vars.get(name) : `\${${name}}`))
    .replace(/\$(\w+)/g, (_, name) => (vars.has(name) ? vars.get(name) : `$${name}`));
}

function parseDockerfile(content) {
  const args = parseDockerArgs(content);
  const refs = [];
  const seen = new Set();
  for (const m of content.matchAll(FROM_REGEX)) {
    const raw = m[1];
    if (raw.startsWith('--')) continue;
    const head = raw.split(/\s+/)[0];
    if (head.startsWith('--')) continue;
    const ref = substituteVars(head, args);
    if (!ref || seen.has(ref)) continue;
    seen.add(ref);
    refs.push(ref);
  }
  return refs;
}

function parseImageLines(content, vars) {
  vars = vars || new Map();
  const refs = [];
  const seen = new Set();
  for (const m of content.matchAll(IMAGE_LINE_REGEX)) {
    const raw = m[1].replace(/['"]/g, '');
    const ref = substituteVars(raw, vars);
    if (!ref || seen.has(ref)) continue;
    seen.add(ref);
    refs.push(ref);
  }
  return refs;
}

function parseCompose(content) {
  return parseImageLines(content, new Map());
}

function parseGitlabCi(content) {
  return parseImageLines(content, parseGitlabVariables(content));
}

function looksLikeImageFile(name) {
  return (
    name === 'Dockerfile' ||
    name === 'Dockerfile.local' ||
    name.startsWith('Dockerfile.') ||
    name.startsWith('docker-compose')
  );
}

function walkTree(dir, out) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const e of entries) {
    const full = path.join(dir, e.name);
    if (e.isDirectory()) {
      if (SKIP_DIRS.has(e.name)) continue;
      walkTree(full, out);
    } else if (looksLikeImageFile(e.name)) {
      out.push(full);
    }
  }
}

function collectFiles(root, rels) {
  const out = [];
  for (const rel of rels) {
    const abs = path.join(root, rel);
    if (!fs.existsSync(abs)) continue;
    const stat = fs.statSync(abs);
    if (stat.isFile()) {
      if (looksLikeImageFile(path.basename(abs))) out.push(abs);
      continue;
    }
    walkTree(abs, out);
  }
  return out;
}

function scanProject(root) {
  const dockerfileSources = collectFiles(root, ['services', 'sdk', 'frontend']);
  const composeSources = collectFiles(root, ['infrastructure']);
  const gitlabCiPath = path.join(root, '.gitlab-ci.yml');

  const seen = new Map();

  const ingest = (filePath, refs) => {
    const source = path.relative(root, filePath);
    for (const ref of refs) {
      if (seen.has(ref)) continue;
      seen.set(ref, { ref, source, mirror: null });
    }
  };

  for (const f of dockerfileSources) {
    ingest(f, parseDockerfile(fs.readFileSync(f, 'utf8')));
  }
  for (const f of composeSources) {
    ingest(f, parseCompose(fs.readFileSync(f, 'utf8')));
  }
  if (fs.existsSync(gitlabCiPath)) {
    ingest(gitlabCiPath, parseGitlabCi(fs.readFileSync(gitlabCiPath, 'utf8')));
  }

  return Array.from(seen.values());
}

module.exports = {
  parseDockerfile,
  parseCompose,
  parseGitlabCi,
  parseDockerArgs,
  parseGitlabVariables,
  substituteVars,
  scanProject,
};