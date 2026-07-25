'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');

const {
  resolveSlug,
  remapServiceVolumes,
  buildManifest,
  rewriteAgentsMd,
  filterCargoWorkspaceMembers,
} = require('./release-transforms');

// ─── resolveSlug ─────────────────────────────────────────────────────────

test('resolveSlug: explicit map wins', () => {
  assert.equal(resolveSlug('iam-service', { 'iam-service': 'iam' }), 'iam');
});

test('resolveSlug: fallback strips -service suffix', () => {
  assert.equal(resolveSlug('audit-service', {}), 'audit');
});

test('resolveSlug: no -service suffix → returned verbatim', () => {
  assert.equal(resolveSlug('prue_backend', {}), 'prue_backend');
});

test('resolveSlug: null slugMap treated as empty', () => {
  assert.equal(resolveSlug('foo-service', null), 'foo');
});

test('resolveSlug: rejects empty svc', () => {
  assert.throws(() => resolveSlug('', {}), /non-empty string/);
  assert.throws(() => resolveSlug(undefined, {}), /non-empty string/);
});

// ─── remapServiceVolumes ─────────────────────────────────────────────────

test('remapServiceVolumes: rewrites iam mount, preserves container path', () => {
  const input = 'volumes:\n  - ../services/iam-service/config:/etc/iam-service\n';
  const { text, changes } = remapServiceVolumes(input, ['iam-service'], { 'iam-service': 'iam' });
  assert.ok(text.includes('./config/iam:/etc/iam-service'));
  assert.ok(!text.includes('../services/iam-service/config'));
  assert.deepEqual(changes, [{ svc: 'iam-service', slug: 'iam', matches: 1 }]);
});

test('remapServiceVolumes: preserves :ro suffix on token mount', () => {
  const input = 'volumes:\n  - ../services/token-service/config:/app/config:ro\n';
  const { text, changes } = remapServiceVolumes(input, ['token-service'], { 'token-service': 'token' });
  assert.ok(text.includes('./config/token:/app/config:ro'), `unexpected output: ${text}`);
  assert.equal(changes[0].matches, 1);
});

test('remapServiceVolumes: shared mount across two services counts each occurrence', () => {
  // iam-service and iam-outbox-publisher both bind-mount the same iam config dir
  const input = [
    'iam-service:',
    '  volumes:',
    '    - ../services/iam-service/config:/etc/iam-service',
    'iam-outbox-publisher:',
    '  volumes:',
    '    - ../services/iam-service/config:/etc/iam-service',
    '',
  ].join('\n');
  const { text, changes } = remapServiceVolumes(input, ['iam-service'], { 'iam-service': 'iam' });
  const matches = text.match(/\.\/config\/iam:/g) || [];
  assert.equal(matches.length, 2);
  assert.equal(changes[0].matches, 2);
});

test('remapServiceVolumes: service not present in yaml reports 0 matches', () => {
  const input = 'volumes:\n  - ../services/iam-service/config:/etc/iam-service\n';
  const { text, changes } = remapServiceVolumes(input, ['iam-service', 'nonexistent-service'], {
    'iam-service': 'iam',
  });
  assert.equal(changes.find(c => c.svc === 'nonexistent-service').matches, 0);
  // iam mount still rewritten
  assert.ok(text.includes('./config/iam:/etc/iam-service'));
});

test('remapServiceVolumes: fallback slug used when svc not in map', () => {
  const input = 'volumes:\n  - ../services/audit-service/config:/etc/audit\n';
  const { text, changes } = remapServiceVolumes(input, ['audit-service'], {});
  assert.ok(text.includes('./config/audit:/etc/audit'));
  assert.equal(changes[0].slug, 'audit');
});

test('remapServiceVolumes: rejects non-string composeText', () => {
  assert.throws(() => remapServiceVolumes(null, [], {}), /must be a string/);
});

test('remapServiceVolumes: rejects non-array services', () => {
  assert.throws(() => remapServiceVolumes('', 'not-array', {}), /must be an array/);
});

// Path-traversal / injection safety: dev prefix is built from `svc` slug via
// template literal, but we only run string.split.join here (not regex), so no
// regex-injection surface. A svc name with `..` still just does literal match.
test('remapServiceVolumes: exotic svc name still literal-matches (no regex injection)', () => {
  const input = 'volumes:\n  - ../services/weird.svc/config:/x\n';
  const { text } = remapServiceVolumes(input, ['weird.svc'], { 'weird.svc': 'weird' });
  assert.ok(text.includes('./config/weird:/x'));
});

// ─── buildManifest ───────────────────────────────────────────────────────

test('buildManifest: happy path', () => {
  const m = buildManifest({
    version: 'v1.0.0',
    registry: 'r.example.com:8000',
    builtAt: '2026-07-25T12:00:00Z',
    gitSha: 'abc1234',
    gitBranch: 'main',
    exportImages: true,
    images: ['r/a:v1', 'r/b:v1'],
  });
  assert.deepEqual(m, {
    version: 'v1.0.0',
    registry: 'r.example.com:8000',
    built_at: '2026-07-25T12:00:00Z',
    git_sha: 'abc1234',
    git_branch: 'main',
    export_images: 1,
    images: ['r/a:v1', 'r/b:v1'],
  });
});

test('buildManifest: exportImages false → export_images: 0', () => {
  const m = buildManifest({
    version: 'v', registry: 'r', builtAt: 't', exportImages: false, images: [],
  });
  assert.equal(m.export_images, 0);
});

test('buildManifest: exportImages numeric 1/0 accepted', () => {
  assert.equal(buildManifest({
    version: 'v', registry: 'r', builtAt: 't', exportImages: 1, images: [],
  }).export_images, 1);
  assert.equal(buildManifest({
    version: 'v', registry: 'r', builtAt: 't', exportImages: 0, images: [],
  }).export_images, 0);
});

test('buildManifest: gitSha defaults to "unknown"', () => {
  const m = buildManifest({
    version: 'v', registry: 'r', builtAt: 't', exportImages: false, images: [],
  });
  assert.equal(m.git_sha, 'unknown');
  assert.equal(m.git_branch, 'unknown');
});

test('buildManifest: rejects missing required fields', () => {
  assert.throws(() => buildManifest({ registry: 'r', builtAt: 't', exportImages: false, images: [] }),
    /version is required/);
  assert.throws(() => buildManifest({ version: 'v', builtAt: 't', exportImages: false, images: [] }),
    /registry is required/);
  assert.throws(() => buildManifest({ version: 'v', registry: 'r', exportImages: false, images: [] }),
    /builtAt is required/);
  assert.throws(() => buildManifest({ version: 'v', registry: 'r', builtAt: 't', images: [] }),
    /exportImages must be boolean/);
  assert.throws(() => buildManifest({ version: 'v', registry: 'r', builtAt: 't', exportImages: true }),
    /images must be an array/);
});

test('buildManifest: images array is copied, not aliased', () => {
  const src = ['a', 'b'];
  const m = buildManifest({
    version: 'v', registry: 'r', builtAt: 't', exportImages: false, images: src,
  });
  src.push('c');
  assert.deepEqual(m.images, ['a', 'b']);
});

// ─── rewriteAgentsMd ─────────────────────────────────────────────────────

test('rewriteAgentsMd: replaces .trae/rules/ → .agents/rules/', () => {
  const before = 'See [.trae/rules/project.md](.trae/rules/project.md) for rules.\n';
  const after = rewriteAgentsMd(before);
  assert.ok(!after.includes('.trae/rules/'));
  assert.ok(after.includes('.agents/rules/'));
});

test('rewriteAgentsMd: replaces .trae/rule/ (dev typo) → .agents/rules/', () => {
  const before = 'And `.trae/rule/legacy.md` too\n';
  const after = rewriteAgentsMd(before);
  assert.ok(after.includes('.agents/rules/legacy.md'));
});

test('rewriteAgentsMd: replaces .trae/skills/ → .agents/skills/', () => {
  const before = '`.trae/skills/foo`\n';
  const after = rewriteAgentsMd(before);
  assert.equal(after, '`.agents/skills/foo`\n');
});

test('rewriteAgentsMd: drops platform skill table rows referencing .trae/skills/', () => {
  const before = [
    '| Skill | Path |',
    '|---|---|',
    '| 加路由 | `.agents/skills/coder-add-route` |',
    '| 同步基础镜像 | `.trae/skills/coder-base-images-sync` |',
    '| 审计服务规范 | `.trae/skills/coder-service-audit` |',
    '| 引导开发环境 | `.trae/skills/coder-dev-bootstrap` |',
    '| 其他 | keep |',
    '',
  ].join('\n');
  const after = rewriteAgentsMd(before);
  assert.ok(!after.includes('同步基础镜像'), `saw drop row: ${after}`);
  assert.ok(!after.includes('审计服务规范'));
  assert.ok(!after.includes('引导开发环境'));
  assert.ok(after.includes('加路由'));
  assert.ok(after.includes('其他 | keep'));
});

test('rewriteAgentsMd: drop-rows execute BEFORE prefix rewrite (order safety)', () => {
  // If drop ran AFTER the .trae→.agents rewrite, the row would already say
  // .agents/skills/ and the drop pattern would miss. This is the regression
  // this test guards.
  const before = '| 同步基础镜像 | ref .trae/skills/coder-base-images-sync |\n' +
                 '| 其他 | ref .trae/skills/other-skill |\n';
  const after = rewriteAgentsMd(before);
  assert.ok(!after.includes('同步基础镜像'));
  assert.ok(after.includes('其他'));
  assert.ok(after.includes('.agents/skills/other-skill'));
});

test('rewriteAgentsMd: rejects non-string', () => {
  assert.throws(() => rewriteAgentsMd(null), /must be a string/);
});

// ─── filterCargoWorkspaceMembers ─────────────────────────────────────────

test('filterCargoWorkspaceMembers: drops services/*, sdk/rust/*, libs/*', () => {
  const before = [
    '[workspace]',
    'members = [',
    '    "libs/common-error",',
    '    "libs/common-db",',
    '    "sdk/rust/audit-service-sdk",',
    '    "services/iam-service",',
    '    "services/iam-outbox-publisher",',
    '    "frontend",',
    '    "tests/e2e",',
    ']',
    '',
  ].join('\n');
  const after = filterCargoWorkspaceMembers(before);
  assert.ok(!after.includes('libs/'));
  assert.ok(!after.includes('sdk/rust/'));
  assert.ok(!after.includes('services/'));
  assert.ok(after.includes('"frontend"'));
  assert.ok(after.includes('"tests/e2e"'));
});

test('filterCargoWorkspaceMembers: preserves comments and non-member lines', () => {
  const before = [
    '# top comment',
    '[workspace]',
    'resolver = "2"',
    'members = [',
    '    "libs/common-error",  # important lib',
    '    "keep-me",',
    ']',
    '',
    '[workspace.dependencies]',
    'serde = "1"',
    '',
  ].join('\n');
  const after = filterCargoWorkspaceMembers(before);
  assert.ok(after.includes('# top comment'));
  assert.ok(after.includes('resolver = "2"'));
  assert.ok(after.includes('"keep-me"'));
  assert.ok(after.includes('[workspace.dependencies]'));
  assert.ok(!after.includes('"libs/common-error"'));
});

test('filterCargoWorkspaceMembers: rejects non-string', () => {
  assert.throws(() => filterCargoWorkspaceMembers(null), /must be a string/);
});
