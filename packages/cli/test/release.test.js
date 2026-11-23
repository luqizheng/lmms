'use strict';

// release.test.js —— 发布链路的不变量
//
// 守三件事：
//   1. 版本真相源唯一：services/*/Cargo.toml 继承 [workspace.package].version，
//      解析后 8 个服务一致（分裂 → VERSION_SPLIT 拒绝发布）
//   2. .image.env 的钉版 = 服务版本 + 方言后缀（部署单元不会拉到别的版本）
//   3. Rust 发布序尊重依赖方向，path 依赖改写/复原无残留

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  sectionKeys,
  readWorkspaceVersion,
  readServiceVersions,
  publishedDirs,
} = require('../lib/service-versions.js');
const { cargoPublishArgs, rewritePathDeps, internalPathDeps, assertOrderRespectsDeps, publishRust, publishNpm, LIBS_ORDER, SDKS_ORDER, NPM_PACKAGES } = require('../lib/sdk-publish.js');
const { readReleaseConfig } = require('../lib/release-toml.js');
const imageEnv = require('../lib/image-env.js');

const REPO_ROOT = path.resolve(__dirname, '../../..');

// ── TOML 解析 ────────────────────────────────────────────────────────────────
test('sectionKeys 只取目标 section 的键，跳过数组/多行值', () => {
  const toml = [
    '[workspace]',
    'members = [',
    '  "a",',
    ']',
    'resolver = "2"',
    '',
    '[workspace.package]',
    'edition = "2021"',
    'version = "1.7.0"',
  ].join('\n');
  const ws = sectionKeys(toml, 'workspace.package');
  assert.equal(ws.get('version'), '1.7.0');
  assert.equal(ws.get('edition'), '2021');
  assert.equal(ws.get('members'), undefined);
});

test('sectionKeys 识别 version.workspace 这类带点的键', () => {
  const toml = ['[package]', 'name = "api_gateway"', 'version.workspace = true'].join('\n');
  const pkg = sectionKeys(toml, 'package');
  assert.equal(pkg.get('version.workspace'), 'true');
  assert.equal(pkg.get('name'), 'api_gateway');
});

// ── workspace 版本 ──────────────────────────────────────────────────────────
test('工作区声明了 [workspace.package].version', () => {
  const v = readWorkspaceVersion(REPO_ROOT);
  assert.ok(v, '根 Cargo.toml 必须有 [workspace.package].version');
  assert.match(v, /^\d+\.\d+\.\d+/);
});

test('8 个发布服务版本一致，且都继承 workspace（没有写死版本）', () => {
  const r = readServiceVersions(REPO_ROOT);
  assert.deepEqual(r.missing, [], '所有服务的 Cargo.toml 都应存在');
  assert.deepEqual(r.splits, [], '服务版本不应分裂');
  assert.ok(r.consistent, `版本应一致，实际 version=${r.version}`);
  assert.equal(r.version, readWorkspaceVersion(REPO_ROOT));
  assert.equal(r.services.length, publishedDirs().length);
  for (const s of r.services) {
    assert.ok(s.inherited, `${s.dir} 应使用 version.workspace = true`);
  }
});

test('版本分裂会被识别（模拟两个服务写死不同版本）', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'lmms-versplit-'));
  try {
    fs.writeFileSync(path.join(tmp, 'Cargo.toml'), '[workspace.package]\nversion = "2.0.0"\n');
    for (const dir of publishedDirs()) {
      const svcDir = path.join(tmp, 'services', dir);
      fs.mkdirSync(path.join(svcDir), { recursive: true });
      // 第一个服务写死 1.0.0，其余继承 2.0.0 → 应报 split
      const body = dir === publishedDirs()[0]
        ? '[package]\nname = "x"\nversion = "1.0.0"\n'
        : '[package]\nname = "x"\nversion.workspace = true\n';
      fs.writeFileSync(path.join(svcDir, 'Cargo.toml'), body);
    }

    const r = readServiceVersions(tmp);
    assert.equal(r.consistent, false);
    assert.equal(r.splits.length, 1);
    assert.equal(r.splits[0].dir, publishedDirs()[0]);
    assert.equal(r.splits[0].version, '1.0.0');
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

// ── 镜像钉版 ────────────────────────────────────────────────────────────────
test('每个部署单元的 .image.env 平台镜像段 = 服务版本 + 方言后缀', () => {
  const r = readServiceVersions(REPO_ROOT);
  const cfg = readReleaseConfig(REPO_ROOT);
  const refs = imageEnv.imageRefs({
    version: r.version,
    dialect: cfg.dialect,
    registry: cfg.registry,
    repo: cfg.repo,
  });
  for (const ref of refs) {
    assert.ok(ref.ref.includes(`:${r.version}`), `${ref.env} 应指向 ${r.version}: ${ref.ref}`);
  }
  // 两个部署单元的平台段必须逐字一致
  for (const unit of imageEnv.DEPLOY_UNITS) {
    const file = path.join(REPO_ROOT, unit.dir, imageEnv.ENV_FILE);
    const vars = imageEnv.parseImageEnv(fs.readFileSync(file, 'utf8'));
    for (const ref of refs) {
      assert.equal(vars.get(ref.env), ref.ref, `${unit.dir} 的 ${ref.env} 应等于 ${ref.ref}`);
    }
  }
});

test('全栈单元的 .image.env 含全部基础镜像变量', () => {
  const full = imageEnv.DEPLOY_UNITS.find((u) => u.base);
  assert.ok(full, '应有 base=true 的部署单元');
  const vars = imageEnv.parseImageEnv(
    fs.readFileSync(path.join(REPO_ROOT, full.dir, imageEnv.ENV_FILE), 'utf8')
  );
  for (const b of imageEnv.BASE_IMAGES) {
    assert.ok(vars.get(b.env), `${full.dir} 缺基础镜像 ${b.env}`);
    assert.match(vars.get(b.env), /:[^:\s]+$/, `${b.env} 必须带 tag`);
    assert.ok(!vars.get(b.env).endsWith(':latest'), `${b.env} 不得指向 :latest`);
  }
});

test('services-only 单元不写基础镜像变量（职责分离）', () => {
  const only = imageEnv.DEPLOY_UNITS.find((u) => !u.base);
  assert.ok(only);
  const vars = imageEnv.parseImageEnv(
    fs.readFileSync(path.join(REPO_ROOT, only.dir, imageEnv.ENV_FILE), 'utf8')
  );
  for (const b of imageEnv.BASE_IMAGES) {
    assert.equal(vars.get(b.env), undefined, `${only.dir} 不应声明基础镜像 ${b.env}`);
  }
});

test('基础镜像段手工改动在重新生成时被保留（升级 postgres 不被覆盖）', () => {
  const custom = 'IMAGE_CONSUL=registry.local/consul:9.9.9\nIMAGE_POSTGRES=registry.local/postgres:17\n';
  const out = imageEnv.buildImageEnv({ version: '1.0.0', dialect: 'pg', includeBase: true, existingRaw: custom });
  assert.ok(out.includes('IMAGE_CONSUL=registry.local/consul:9.9.9'), '手工改的 consul 应保留');
  assert.ok(out.includes('IMAGE_POSTGRES=registry.local/postgres:17'), '手工升的 postgres 应保留');
  assert.ok(out.includes('IMAGE_REDIS=zhcoder-docker-registry.com:8000/official/redis:7-alpine'), '未改的用默认值');
});

test('compose 引用的 IMAGE_* 变量集合 == .image.env 声明的变量集合（遍历全部部署单元）', () => {
  for (const unit of imageEnv.DEPLOY_UNITS) {
    const dir = path.join(REPO_ROOT, unit.dir);
    const declared = new Set(
      fs.readFileSync(path.join(dir, imageEnv.ENV_FILE), 'utf8')
        .split('\n').map((l) => l.match(/^(IMAGE_[A-Z_]+)=/)?.[1]).filter(Boolean)
    );
    const used = new Set();
    for (const f of fs.readdirSync(dir).filter((n) => /^docker-compose.*\.ya?ml$/.test(n))) {
      const text = fs.readFileSync(path.join(dir, f), 'utf8');
      for (const m of text.matchAll(/image:\s*"\$\{(IMAGE_[A-Z_]+):\?/g)) used.add(m[1]);
    }
    assert.deepEqual([...used].sort(), [...declared].sort(), `${unit.dir}: compose ↔ .image.env 变量不一致`);
  }
});

// ── Rust 发布：path 依赖改写 ─────────────────────────────────────────────────
test('rewritePathDeps 把 path 依赖改成 version + registry', () => {
  const before = [
    'digua_common_error = { path = "../common-error" }',
    'digua_common_db = { path = "../common-db", features = ["postgres"] }',
  ].join('\n');
  const after = rewritePathDeps(before, {
    crateVersions: { 'common-error': '1.6.0', 'common-db': '1.7.0' },
    registry: 'kellnr',
    pkgNames: { 'common-error': 'digua_common_error', 'common-db': 'digua_common_db' },
  });
  assert.ok(after.includes('digua_common_error = { version = "1.6.0", registry = "kellnr" }'));
  assert.ok(after.includes('digua_common_db = { version = "1.7.0", registry = "kellnr", features = ["postgres"] }'));
  assert.ok(!after.includes('path ='));
});

test('cargoPublishArgs 拒绝空 registry（禁止隐式回退 crates.io）', () => {
  assert.throws(
    () => cargoPublishArgs('digua_common_error', null, false),
    (err) => err.code === 'RUST_REGISTRY_UNSET',
  );
  assert.throws(
    () => cargoPublishArgs('digua_common_error', '', false),
    (err) => err.code === 'RUST_REGISTRY_UNSET',
  );
  const args = cargoPublishArgs('digua_common_error', 'kellnr', false);
  assert.ok(args.includes('publish'));
  assert.deepEqual(args.slice(-2), ['--registry', 'kellnr']);
  const dry = cargoPublishArgs('digua_common_error', 'kellnr', true);
  assert.ok(dry.includes('--dry-run') && dry.includes('--registry'));
});

test('rewritePathDeps supports the default crates.io registry without a private registry alias', () => {
  const before = 'digua_common_error = { path = "../common-error" }';
  const after = rewritePathDeps(before, {
    crateVersions: { 'common-error': '1.6.0' },
    registry: null,
    pkgNames: { 'common-error': 'digua_common_error' },
  });
  assert.equal(after, 'digua_common_error = { version = "1.6.0" }');
  assert.ok(!after.includes('registry ='));
});

test('internalPathDeps 找出指向仓内 crate 的依赖，且排除自身', () => {
  const toml = [
    'digua_common_error = { path = "../common-error" }',
    'digua_common = { path = "../common" }',
    'serde = { version = "1" }',
  ].join('\n');
  const deps = internalPathDeps(toml, ['common-error', 'common', 'common-db'], (d) => `digua_${d.replace(/-/g, '_')}`, 'common');
  assert.deepEqual(deps.sort(), ['common-error']);
});

test('发布序违反依赖方向时报 CRATE_ORDER_VIOLATION', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'lmms-order-'));
  try {
    const base = path.join(tmp, 'libs');
    // common-event 依赖 common-db，但顺序把 common-event 排在前面
    fs.mkdirSync(path.join(base, 'common-event'), { recursive: true });
    fs.mkdirSync(path.join(base, 'common-db'), { recursive: true });
    fs.writeFileSync(path.join(base, 'common-event', 'Cargo.toml'),
      'name = "digua_common_event"\ndigua_common_db = { path = "../common-db" }\n');
    fs.writeFileSync(path.join(base, 'common-db', 'Cargo.toml'), 'name = "digua_common_db"\n');

    assert.throws(
      () => assertOrderRespectsDeps([
        { dir: 'common-event', family: 'libs', base },
        { dir: 'common-db', family: 'libs', base },
      ]),
      /publish order/,
    );
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

test('LIBS_ORDER 里 common-db 排在依赖它的 crate 之前', () => {
  const idx = (n) => LIBS_ORDER.indexOf(n);
  assert.ok(idx('common-db') >= 0);
  for (const dependent of ['common', 'common-event']) {
    if (idx(dependent) >= 0) {
      assert.ok(idx('common-db') < idx(dependent), `common-db 必须排在 ${dependent} 之前`);
    }
  }
});

// ── 发布清单与仓库实际包结构一致 ─────────────────────────────────────────────
test('npm 发布清单排除内部包，且每个条目都是可公开发布的真实包', () => {
  const names = NPM_PACKAGES.map((p) => p.name);
  assert.ok(!names.includes('@digua/cli'), '内部 @digua/cli 不得进入公开发布清单');
  for (const p of NPM_PACKAGES) {
    const pkgPath = path.join(REPO_ROOT, p.dir, 'package.json');
    assert.ok(fs.existsSync(pkgPath), `${p.dir}/package.json 必须存在`);
    const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
    assert.equal(pkg.name, p.name, `${p.dir} 的 package name 与发布清单一致`);
    assert.notEqual(pkg.private, true, `${p.dir} 标记了 private，不能进入公开发布清单`);
  }
});

test('Rust 发布清单条目都有 description/license（crates.io 必填）', () => {
  const dirs = [
    ...LIBS_ORDER.map((d) => `libs/${d}`),
    ...SDKS_ORDER.map((d) => `sdk/rust/${d}`),
  ];
  for (const dir of dirs) {
    const tomlPath = path.join(REPO_ROOT, dir, 'Cargo.toml');
    assert.ok(fs.existsSync(tomlPath), `${dir}/Cargo.toml 必须存在`);
    const toml = fs.readFileSync(tomlPath, 'utf8');
    assert.match(toml, /^description\s*=/m, `${dir} 缺少 description`);
    assert.match(toml, /^license\s*=/m, `${dir} 缺少 license`);
  }
});

// ── npm 包名与许可证 ────────────────────────────────────────────────────────
test('所有公开 npm 包使用 @digua/ scope（未加 scope 的名称在 npmjs 已被占用）', () => {
  for (const p of NPM_PACKAGES) {
    assert.match(p.name, /^@digua\//, `${p.dir} 的包名 ${p.name} 必须使用 @digua/ scope`);
  }
  const names = NPM_PACKAGES.map((p) => p.name);
  assert.equal(new Set(names).size, names.length, '发布清单不得有重复包名');
});

test('所有公开 npm 包声明 Apache-2.0，且 publishConfig 指向内部 npm Registry', () => {
  for (const p of NPM_PACKAGES) {
    const pkg = JSON.parse(fs.readFileSync(path.join(REPO_ROOT, p.dir, 'package.json'), 'utf8'));
    assert.equal(pkg.license, 'Apache-2.0', `${p.name} 的 license 必须是 Apache-2.0`);
    assert.equal(pkg.publishConfig?.registry, 'http://192.168.2.12:14873/', `${p.name} 必须发布到内部 npm Registry`);
  }
});

test('仓库根存在 Apache-2.0 LICENSE', () => {
  const licensePath = path.join(REPO_ROOT, 'LICENSE');
  assert.ok(fs.existsSync(licensePath), '仓库根必须有 LICENSE');
  assert.match(fs.readFileSync(licensePath, 'utf8'), /Apache License\s+Version 2\.0/);
});

// ── 发布 workflow 触发与 tag 校验 ───────────────────────────────────────────
test('public-release workflow 支持 workflow_dispatch 指定已存在的 tag', () => {
  const wf = fs.readFileSync(path.join(REPO_ROOT, '.github', 'workflows', 'public-release.yml'), 'utf8');
  assert.match(wf, /^\s{2}workflow_dispatch:/m, '必须支持 workflow_dispatch');
  assert.match(wf, /^\s{6}tag:/m, 'workflow_dispatch 必须有 tag 输入');
  assert.match(wf, /required:\s*true/, 'tag 输入必须必填');
});

test('public-release 的手动补发仍校验 tag 属于 main，且发布 job 不在 PR 上运行', () => {
  const wf = fs.readFileSync(path.join(REPO_ROOT, '.github', 'workflows', 'public-release.yml'), 'utf8');
  assert.match(wf, /\^v\[0-9\]\+\\\.\[0-9\]\+\\\.\[0-9\]\+\$/, '必须校验 vMAJOR.MINOR.PATCH 格式');
  assert.match(wf, /git merge-base --is-ancestor/, '必须校验 tag 的 commit 可从 main 到达');
  assert.match(wf, /inputs\.tag/, 'dispatch 路径必须读取手动输入的 tag');
  const jobGuards = wf.match(/^\s{2}publish-[a-z]+:\n\s{4}if: github\.event_name != 'pull_request'/gm) || [];
  assert.equal(jobGuards.length, 3, '三个发布 job 都必须排除 pull_request 事件');
});


// ── release.toml 不带版本 ───────────────────────────────────────────────────
test('release.toml 不再持有 version（版本归 Cargo.toml，避免双真相源）', () => {
  const cfg = readReleaseConfig(REPO_ROOT);
  assert.equal(cfg.version, undefined);
  assert.ok(['pg', 'mysql'].includes(cfg.dialect));
  assert.ok(cfg.registry);
  assert.ok(cfg.repo);
});

test('rewritePathDeps 对自带 version 的 path 依赖不产生重复 version 键（1.8.1 发布实证 bug）', () => {
  const before = [
    'digua_common_error = { path = "../common-error", version = "1.8.1" }',
    'digua_common_telemetry = { path = "../common-telemetry", default-features = false, version = "1.8.1" }',
  ].join('\n');
  const after = rewritePathDeps(before, {
    crateVersions: { 'common-error': '1.8.1', 'common-telemetry': '1.8.1' },
    registry: 'kellnr',
    pkgNames: { 'common-error': 'digua_common_error', 'common-telemetry': 'digua_common_telemetry' },
  });
  for (const line of after.split('\n').filter((l) => l.includes('digua_'))) {
    assert.equal(line.match(/version =/g)?.length, 1, '重复 version 键: ' + line);
    assert.ok(!line.includes('path ='), '残留 path: ' + line);
  }
  assert.ok(after.includes('digua_common_error = { version = "1.8.1", registry = "kellnr" }'));
  assert.ok(after.includes('digua_common_telemetry = { version = "1.8.1", registry = "kellnr", default-features = false }'));
});
