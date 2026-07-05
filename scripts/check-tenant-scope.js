#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

const SERVICES_DIR = 'services';
const RUST_EXT = '.rs';
const HANDLER_RE = /(?:\/\/[^\n]*\n)*\s*(?:pub\s+)?async\s+fn\s+(\w+)\s*\(/g;
const HAS_TENANT_SCOPE = /TenantScope/;
const HAS_FILTER = /and_tenant|tenant_id|TenantScope::CrossTenant/;
const EXEMPT = /check-tenant-scope:\s*delegated/;

function walkDir(dir) {
  const results = [];
  if (!fs.existsSync(dir)) return results;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory() && !entry.name.startsWith('.') && entry.name !== 'node_modules' && entry.name !== 'target') {
      results.push(...walkDir(full));
    } else if (entry.isFile() && entry.name.endsWith(RUST_EXT)) {
      results.push(full);
    }
  }
  return results;
}

function lineOf(code, pos) {
  return code.slice(0, pos).split('\n').length;
}

function findMatchingBrace(code, openPos) {
  let depth = 0;
  for (let i = openPos; i < code.length; i++) {
    if (code[i] === '{') depth++;
    else if (code[i] === '}') {
      depth--;
      if (depth === 0) return i;
    }
  }
  return code.length - 1;
}

function extractFunctions(code) {
  const fns = [];
  let m;
  const re = new RegExp(HANDLER_RE.source, HANDLER_RE.flags);
  while ((m = re.exec(code)) !== null) {
    const matchedText = m[0];
    const name = m[1];
    const sigStart = m.index;
    const sigEnd = m.index + matchedText.length;

    // capture leading comments (lines directly above, separated only by whitespace)
    const before = code.slice(0, sigStart);
    const beforeComments = before.match(/((?:\/\/[^\n]*\n)+)\s*$/);
    const matchedComments = matchedText.match(/^((?:\/\/[^\n]*\n)+)/);
    const comment = (beforeComments ? beforeComments[1] : '') + (matchedComments ? matchedComments[1] : '');

    // find opening brace
    let bracePos = sigEnd;
    while (bracePos < code.length && code[bracePos] !== '{') bracePos++;
    if (bracePos >= code.length) continue;

    const bodyEnd = findMatchingBrace(code, bracePos);
    const body = code.slice(bracePos, bodyEnd + 1);
    const sig = code.slice(sigStart, bracePos);

    const line = lineOf(code, sigStart);

    fns.push({ name, sig, body, line, comment });
  }
  return fns;
}

function checkFile(filePath) {
  const code = fs.readFileSync(filePath, 'utf8');
  const fns = extractFunctions(code);
  const errors = [];
  for (const fn of fns) {
    if (EXEMPT.test(fn.comment)) continue;
    if (HAS_TENANT_SCOPE.test(fn.sig) && !HAS_FILTER.test(fn.body)) {
      errors.push(`${filePath}:${fn.line}: handler "${fn.name}" takes TenantScope but SQL has no tenant_id filter`);
    }
  }
  return errors;
}

function main() {
  const repoRoot = path.resolve(__dirname, '..');
  const servicesDir = path.join(repoRoot, SERVICES_DIR);
  const files = walkDir(servicesDir);
  let allErrors = [];
  for (const f of files) {
    allErrors.push(...checkFile(f));
  }
  if (allErrors.length > 0) {
    console.error('check-tenant-scope: FAIL');
    allErrors.forEach(e => console.error('  ' + e));
    process.exit(1);
  }
  console.log('check-tenant-scope: OK');
  process.exit(0);
}

main();
