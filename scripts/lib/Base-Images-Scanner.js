'use strict';
// Base-Images-Scanner.js
// 扫描 Dockerfile（含 services/ sdk/）、docker-compose*.yml（infrastructure/）、
// 以及根目录 .gitlab-ci.yml 中的基础镜像引用；对 Dockerfile 的 ARG 与 gitlab-ci 的 variables
// 做变量替换，把 ${NAME} / $NAME 展开成真实 ref。

const fs = require('node:fs');
const path = require('node:path');

const FROM_REGEX = /^FROM\s+(\S+)/gm;
const ARG_REGEX = /^ARG\s+(\w+)\s*=\s*(\S+)/gm;
const IMAGE_LINE_REGEX = /^\s*image:\s*(\S+)/gm;
const ENV_LINE_REGEX = /^\s*(?:export\s+)?([A-Z_][A-Z0-9_]*)\s*=\s*(['"]?)(.*?)\2\s*$/;

const CI_VAR_LINE_REGEX = /^\s{2,}(\w+):\s*(\S+)/;
const VARIABLES_BLOCK_START = /^variables:\s*$/;

const SKIP_DIRS = new Set(['target', 'node_modules', '.git', 'reports', '.codegraph']);

// Parse a docker-compose `.env`-style file (KEY=VALUE, optional `export ` prefix,
// optional single/double quotes, `#` comments, blank lines). Values are trimmed.
// Returns a Map<string,string> with first occurrence winning; subsequent
// duplicates are ignored (docker compose / dotenv semantics).
function parseEnvFile(content) {
  const vars = new Map();
  for (const raw of content.split('\n')) {
    const line = raw.replace(/\s+$/, '');
    if (!line || line.trimStart().startsWith('#')) continue;
    const m = line.match(ENV_LINE_REGEX);
    if (!m) continue;
    const name = m[1];
    if (vars.has(name)) continue;
    vars.set(name, m[3]);
  }
  return vars;
}

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
  // Iterate to a fixed point so transitive references resolve
  // (e.g. `image: $BUILDER_RUST_IMAGE` where BUILDER_RUST_IMAGE itself
  // contains `${CODER_BUILDER_REGISTRY}/...`). Cap at 10 iterations to
  // defend against accidental self-referential cycles.
  let prev = null;
  let cur = ref;
  for (let i = 0; cur !== prev && i < 10; i++) {
    prev = cur;
    cur = cur
      .replace(/\$\{(\w+)\}/g, (_, name) => (vars.has(name) ? vars.get(name) : `\${${name}}`))
      .replace(/\$(\w+)/g, (_, name) => (vars.has(name) ? vars.get(name) : `$${name}`));
  }
  return cur;
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

function parseCompose(content, envVars) {
  // Compose files may declare `build:` for services whose image is built
  // locally (not pulled from a registry). Those images should be excluded
  // from the pull manifest — they'll be built by `docker compose build`.
  const lines = content.split('\n');
  const servicesWithBuild = new Set();
  let currentService = null;

  // Pass 1: find services that declare `build:` at service-child indent.
  for (const line of lines) {
    const trimmed = line.trimStart();
    const indent = line.length - trimmed.length;
    // service-level key (2-space indent under top-level `services:`)
    if (indent === 2 && /^\w[\w-]*:/.test(trimmed)) {
      currentService = trimmed.replace(/:$/, '');
    }
    // build: at 4-space indent (direct child of a service)
    if (indent === 4 && currentService && trimmed.startsWith('build:')) {
      servicesWithBuild.add(currentService);
    }
  }

  // Pass 2: collect image refs, skipping services with build.
  const refs = [];
  const seen = new Set();
  currentService = null;
  for (const line of lines) {
    const trimmed = line.trimStart();
    const indent = line.length - trimmed.length;
    if (indent === 2 && /^\w[\w-]*:/.test(trimmed)) {
      currentService = trimmed.replace(/:$/, '');
    }
    if (indent === 4 && currentService && !servicesWithBuild.has(currentService)) {
      const m = trimmed.match(/^image:\s*(\S+)/);
      if (m) {
        const raw = m[1].replace(/['"]/g, '');
        // Substitute ${VAR} / $VAR from the surrounding .env so a ref like
        // `${BASE_REGISTRY}mysql:8.0` collapses to the short form `mysql:8.0`
        // and the writer can re-prefix it with the mirror.
        const ref = envVars ? substituteVars(raw, envVars) : raw;
        if (ref && !seen.has(ref)) {
          seen.add(ref);
          refs.push(ref);
        }
      }
    }
  }
  return refs;
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
  const dockerfileSources = collectFiles(root, ['services', 'sdk']);
  const composeSources = collectFiles(root, ['infrastructure']);
  const gitlabCiPath = path.join(root, '.gitlab-ci.yml');

  // Docker compose looks for `.env` next to the compose file; mirror that here
  // so ${BASE_REGISTRY}mysql:8.0 etc. collapse to short refs. Falls back to
  // empty map if the file is absent (older setups without BASE_REGISTRY).
  const envPath = path.join(root, 'infrastructure', '.env');
  const composeEnv = fs.existsSync(envPath)
    ? parseEnvFile(fs.readFileSync(envPath, 'utf8'))
    : new Map();

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
    ingest(f, parseCompose(fs.readFileSync(f, 'utf8'), composeEnv));
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
  parseEnvFile,
  parseDockerArgs,
  parseGitlabVariables,
  substituteVars,
  scanProject,
};