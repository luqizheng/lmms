'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { syncGitlabCI, normalizeLines, applyGitlabCI } = require('./setup-wizard');

const SAMPLE_CI = [
  'stages:',
  '  - test',
  '  - build',
  '',
  'variables:',
  '  GLOBAL_VAR: "hello"',
  '  # keep this comment',
  '  OTHER: "leave-me"',
  '',
  'before_script:',
  '  - echo hi',
  '',
].join('\n');

const CFG_BASE = {
  baseRegistry: 'https://reg.example.com',
  crateRegistry: 'https://rsproxy.cn/index/',
  crateToken: 'tk',
  mavenRegistry: '',
  mavenAuthMode: 'none',
  mavenToken: '',
  npmRegistry: '',
  npmToken: '',
};

function silenceStdout(fn) {
  const orig = console.log;
  console.log = () => {};
  try { return fn(); } finally { console.log = orig; }
}

// ─── normalizeLines ─────────────────────────────────────────────────────

test('normalizeLines: empty array stays empty', () => {
  assert.deepEqual(normalizeLines([]), []);
});

test('normalizeLines: no trailing blank lines → unchanged', () => {
  assert.deepEqual(normalizeLines(['a', 'b', 'c']), ['a', 'b', 'c']);
});

test('normalizeLines: one trailing blank line → popped', () => {
  assert.deepEqual(normalizeLines(['a', 'b', '']), ['a', 'b']);
});

test('normalizeLines: many trailing blanks (empty / spaces / tabs) → all popped', () => {
  assert.deepEqual(normalizeLines(['a', 'b', '', '   ', '\t', '']), ['a', 'b']);
});

test('normalizeLines: blank lines in the middle are preserved', () => {
  assert.deepEqual(normalizeLines(['a', '', 'b', '']), ['a', '', 'b']);
});

// ─── syncGitlabCI ───────────────────────────────────────────────────────

test('syncGitlabCI: missing variables: section → missingSection=true, content unchanged', () => {
  const ci = 'stages:\n  - test\n';
  const r = syncGitlabCI(ci, CFG_BASE);
  assert.equal(r.missingSection, true);
  assert.equal(r.changed, false);
  assert.equal(r.newContent, ci);
});

test('syncGitlabCI: first run inserts CODER_* lines and ends with single \\n', () => {
  const r = syncGitlabCI(SAMPLE_CI, CFG_BASE);
  assert.equal(r.changed, true);
  assert.equal(r.missingSection, false);
  assert.ok(r.newContent.endsWith('\n'), 'must end with single \\n');
  assert.ok(!r.newContent.endsWith('\n\n'), 'must NOT end with multiple \\n');
  assert.match(r.newContent, /  CODER_BASE_REGISTRY: "https:\/\/reg\.example\.com"/);
  assert.match(r.newContent, /  CODER_CRATE_TOKEN: "\$CRATE_TOKEN"/);
});

test('syncGitlabCI: second run is byte-identical (idempotent)', () => {
  const first = syncGitlabCI(SAMPLE_CI, CFG_BASE);
  const second = syncGitlabCI(first.newContent, CFG_BASE);
  assert.equal(second.changed, false);
  assert.equal(second.newContent, first.newContent);
});

test('syncGitlabCI: 5 successive runs stay byte-identical (regression: trailing blank line bug)', () => {
  let content = SAMPLE_CI;
  let prev = null;
  for (let i = 0; i < 5; i++) {
    const r = syncGitlabCI(content, CFG_BASE);
    if (prev !== null) {
      assert.equal(r.newContent, prev, `iteration ${i}: content drifted`);
    }
    prev = r.newContent;
    content = r.newContent;
  }
  assert.ok(content.endsWith('\n') && !content.endsWith('\n\n'));
});

test('syncGitlabCI: source with trailing \\n\\n → output single \\n', () => {
  const ci = SAMPLE_CI + '\n';
  const r = syncGitlabCI(ci, CFG_BASE);
  assert.equal(r.changed, true);
  assert.ok(r.newContent.endsWith('\n'));
  assert.ok(!r.newContent.endsWith('\n\n'));
});

test('syncGitlabCI: source with trailing \\n\\n\\n\\n → output single \\n', () => {
  const ci = SAMPLE_CI + '\n\n\n';
  const r = syncGitlabCI(ci, CFG_BASE);
  assert.ok(r.newContent.endsWith('\n'));
  assert.ok(!r.newContent.endsWith('\n\n'));
});

test('syncGitlabCI: source without trailing newline → output single \\n', () => {
  const ci = SAMPLE_CI.replace(/\n$/, '');
  const r = syncGitlabCI(ci, CFG_BASE);
  assert.ok(r.newContent.endsWith('\n'));
});

test('syncGitlabCI: preserves non-CODER lines inside variables: block (vars / comments / blanks)', () => {
  const r = syncGitlabCI(SAMPLE_CI, CFG_BASE);
  assert.match(r.newContent, /  GLOBAL_VAR: "hello"/);
  assert.match(r.newContent, /  OTHER: "leave-me"/);
  assert.match(r.newContent, /  # keep this comment/);
});

test('syncGitlabCI: cfg all-empty → only header comment, no CODER_* lines', () => {
  const cfg = Object.fromEntries(Object.keys(CFG_BASE).map((k) => [k, '']));
  const r = syncGitlabCI(SAMPLE_CI, cfg);
  assert.equal(r.changed, true);
  assert.match(r.newContent, /  # --- setup-wizard 同步/);
  const coderLines = r.newContent
    .split('\n')
    .filter((l) => /^\s+CODER_\w+:\s/.test(l));
  assert.equal(coderLines.length, 0);
});

test('syncGitlabCI: re-running never accumulates the wizard marker line', () => {
  const r1 = syncGitlabCI(SAMPLE_CI, CFG_BASE);
  const r2 = syncGitlabCI(r1.newContent, CFG_BASE);
  const markers = r2.newContent
    .split('\n')
    .filter((l) => l.includes('# --- setup-wizard'));
  assert.equal(markers.length, 1);
});

// ─── applyGitlabCI (fs wrapper) ─────────────────────────────────────────

test('applyGitlabCI: two successive writes leave file byte-identical', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'wizard-test-'));
  const ciPath = path.join(tmp, '.gitlab-ci.yml');
  fs.writeFileSync(ciPath, SAMPLE_CI);

  silenceStdout(() => {
    applyGitlabCI(CFG_BASE, ciPath);
    const first = fs.readFileSync(ciPath, 'utf8');
    applyGitlabCI(CFG_BASE, ciPath);
    const second = fs.readFileSync(ciPath, 'utf8');
    assert.equal(second, first);
    assert.ok(!second.endsWith('\n\n'));
  });

  fs.rmSync(tmp, { recursive: true, force: true });
});

test('applyGitlabCI: missing file → returns false, does not throw', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'wizard-test-'));
  const ciPath = path.join(tmp, 'nonexistent.yml');
  const ok = silenceStdout(() => applyGitlabCI(CFG_BASE, ciPath));
  assert.equal(ok, false);
  fs.rmSync(tmp, { recursive: true, force: true });
});
