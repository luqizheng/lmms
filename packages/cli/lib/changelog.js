'use strict';

// dist CHANGELOG.md ledger. Entries look like `## v1.0.0 (2026-07-12)`.
// CRLF-safe: the detected EOL is reused when splicing new text in.

const ENTRY_RE = /^##[ \t]+v?(\d+\.\d+\.\d+)/m;

function escapeRe(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function detectEol(text) {
  return text.includes('\r\n') ? '\r\n' : '\n';
}

// Version string ('1.0.0') of the first entry; null when there is none.
function readTopVersion(text) {
  const m = ENTRY_RE.exec(text);
  return m ? m[1] : null;
}

function hasVersion(text, imageTag) {
  const re = new RegExp(`^##[ \\t]+v?${escapeRe(imageTag)}(?![.\\d])`, 'm');
  return re.test(text);
}

// Insert a new entry above the first existing one; without prior entries it
// goes right after the `# Changelog` heading. Body newlines are normalized to
// the document's EOL.
function insertEntry(text, { version, date, body }) {
  const eol = detectEol(text);
  const bodyText = String(body).split(/\r?\n/).join(eol);
  const entry = `## v${version} (${date})${eol}${eol}${bodyText}${eol}${eol}`;

  const existing = ENTRY_RE.exec(text);
  if (existing) {
    const idx = existing.index;
    return text.slice(0, idx) + entry + text.slice(idx);
  }

  const heading = /^# Changelog[^\r\n]*/m.exec(text);
  if (heading) {
    const afterHeading = heading.index + heading[0].length;
    return text.slice(0, afterHeading) + eol + eol + entry + text.slice(afterHeading).replace(/^(\r?\n)+/, '');
  }
  return entry + text;
}

module.exports = { readTopVersion, hasVersion, insertEntry };
