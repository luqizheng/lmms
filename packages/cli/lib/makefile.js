'use strict';

// dist-template/Makefile version bookkeeping. Pure string -> string transforms
// are unit-testable; the IO wrapper lives at the bottom.
// CRLF-safe: line regexes use [^\r\n]* so a Windows checkout keeps its \r\n.

const fs = require('node:fs');

const IMAGE_TAG_LINE_RE = /^[ \t]*IMAGE_TAG[ \t]*\??=[^\r\n]*/m;
const BANNER_RE = /(coder-micro-services )v\d+(?:\.\d+)*/;

// Read the current IMAGE_TAG default value ('1.0.0'). Returns null when absent.
function readImageTag(makefileText) {
  const m = IMAGE_TAG_LINE_RE.exec(makefileText);
  if (!m) return null;
  return m[0].split('=')[1].trim();
}

// Rewrite the IMAGE_TAG line and the help banner to the given (no-v) version.
// Idempotent: applying it twice yields the same text.
function applyVersion(makefileText, imageTag) {
  let out = makefileText;
  if (IMAGE_TAG_LINE_RE.test(out)) {
    out = out.replace(IMAGE_TAG_LINE_RE, `IMAGE_TAG ?= ${imageTag}`);
  }
  out = out.replace(BANNER_RE, `$1v${imageTag}`);
  return out;
}

// Returns true when the file content changed (and was written back).
function bumpMakefileFile(makefilePath, imageTag) {
  const before = fs.readFileSync(makefilePath, 'utf-8');
  const after = applyVersion(before, imageTag);
  if (after !== before) fs.writeFileSync(makefilePath, after, 'utf-8');
  return after !== before;
}

module.exports = { readImageTag, applyVersion, bumpMakefileFile };
