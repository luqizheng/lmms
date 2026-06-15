# Base Images Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把硬编码的 `PRE_PULL_IMAGES` 替换为源码驱动的 `infrastructure/base-images.json`，并支持 mirror 拉取 + 自动 retag。

**Architecture:** 新增 `Base-Images-Scanner.js`（纯函数，从 Dockerfile/compose 提取镜像）+ `Base-Images-Writer.js`（合并旧 JSON 的 mirror 字段）+ `base-images-sync.js`（CLI 入口）；修改 `Docker-Helper.prePull()` 读取 JSON，支持 mirror 回源策略。Skill `coder-base-images-sync` 通过现有 `.trae/skills/` 体系注册。

**Tech Stack:** Node.js 内置模块（`node:fs` / `node:path` / `node:child_process`）、原生 `node:test` runner。

---

## File Structure

| 文件 | 职责 |
|---|---|
| `scripts/lib/Base-Images-Scanner.js` | 扫描 FROM/compose image，纯函数 |
| `scripts/lib/Base-Images-Writer.js` | 合并旧 mirror，写入 JSON |
| `scripts/base-images-sync.js` | CLI 入口：scan → merge → write |
| `infrastructure/base-images.json` | 镜像清单 |
| `scripts/lib/Docker-Helper.js` | 修改 prePull 读 JSON + mirror 处理 |
| `.trae/skills/coder-base-images-sync/SKILL.md` | Skill 说明 |
| `.trae/rules/project_rules.md` | 路径 → skill 映射 |
| `AGENTS.md` | Skill 索引 |
| `scripts/lib/__tests__/base-images-scanner.test.js` | scanner 单测 |

---

### Task 1: Scanner 解析 Dockerfile FROM

**Files:**
- Create: `scripts/lib/Base-Images-Scanner.js`
- Create: `scripts/lib/__tests__/base-images-scanner.test.js`

- [ ] **Step 1: 写失败测试**

```js
// scripts/lib/__tests__/base-images-scanner.test.js
const test = require('node:test');
const assert = require('node:assert/strict');
const { parseDockerfile } = require('../Base-Images-Scanner.js');

test('parseDockerfile extracts single FROM', () => {
  const content = 'FROM alpine:3.21\nRUN apk add curl\n';
  const refs = parseDockerfile(content);
  assert.deepEqual(refs, ['alpine:3.21']);
});

test('parseDockerfile extracts multi-stage FROM', () => {
  const content = [
    'FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder',
    'WORKDIR /app',
    'FROM alpine:3.21',
  ].join('\n');
  const refs = parseDockerfile(content);
  assert.deepEqual(refs, [
    'zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl',
    'alpine:3.21',
  ]);
});

test('parseDockerfile deduplicates identical refs', () => {
  const content = 'FROM alpine:3.21\nFROM alpine:3.21\n';
  const refs = parseDockerfile(content);
  assert.deepEqual(refs, ['alpine:3.21']);
});
```

- [ ] **Step 2: 跑测试确认失败**

```bash
node --test scripts/lib/__tests__/base-images-scanner.test.js
```

Expected: FAIL（模块不存在）。

- [ ] **Step 3: 实现 scanner**

```js
// scripts/lib/Base-Images-Scanner.js
'use strict';

const FROM_REGEX = /^FROM\s+(\S+)/gm;

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

module.exports = { parseDockerfile };
```

- [ ] **Step 4: 跑测试确认通过**

```bash
node --test scripts/lib/__tests__/base-images-scanner.test.js
```

Expected: PASS（3 cases）。

- [ ] **Step 5: 提交**

```bash
git add scripts/lib/Base-Images-Scanner.js scripts/lib/__tests__/base-images-scanner.test.js
git commit -m "feat(scripts): add dockerfile FROM parser"
```

---

### Task 2: Scanner 解析 docker-compose image

**Files:**
- Modify: `scripts/lib/Base-Images-Scanner.js`
- Modify: `scripts/lib/__tests__/base-images-scanner.test.js`

- [ ] **Step 1: 加失败测试**

在 `base-images-scanner.test.js` 末尾追加：

```js
const { parseCompose } = require('../Base-Images-Scanner.js');

test('parseCompose extracts image fields', () => {
  const content = [
    'services:',
    '  mysql:',
    '    image: mysql:8.0',
    '  redis:',
    '    image: redis:7-alpine',
  ].join('\n');
  const refs = parseCompose(content);
  assert.deepEqual(refs, ['mysql:8.0', 'redis:7-alpine']);
});

test('parseCompose deduplicates', () => {
  const content = 'services:\n  a:\n    image: alpine:3.21\n  b:\n    image: alpine:3.21\n';
  const refs = parseCompose(content);
  assert.deepEqual(refs, ['alpine:3.21']);
});
```

- [ ] **Step 2: 跑测试确认失败**

```bash
node --test scripts/lib/__tests__/base-images-scanner.test.js
```

Expected: FAIL（`parseCompose` 未定义）。

- [ ] **Step 3: 实现 parseCompose**

修改 `scripts/lib/Base-Images-Scanner.js`，在 `module.exports` 前加：

```js
const COMPOSE_IMAGE_REGEX = /^\s*image:\s*(\S+)/gm;

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
```

`module.exports` 改为：

```js
module.exports = { parseDockerfile, parseCompose };
```

- [ ] **Step 4: 跑测试**

```bash
node --test scripts/lib/__tests__/base-images-scanner.test.js
```

Expected: PASS（5 cases）。

- [ ] **Step 5: 提交**

```bash
git add scripts/lib/Base-Images-Scanner.js scripts/lib/__tests__/base-images-scanner.test.js
git commit -m "feat(scripts): add compose image parser"
```

---

### Task 3: Scanner 扫描目录收集镜像

**Files:**
- Modify: `scripts/lib/Base-Images-Scanner.js`
- Modify: `scripts/lib/__tests__/base-images-scanner.test.js`

- [ ] **Step 1: 加失败测试**

```js
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { scanProject } = require('../Base-Images-Scanner.js');

test('scanProject walks dockerfiles and compose files', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'imgscan-'));
  fs.mkdirSync(path.join(tmp, 'services', 'a'), { recursive: true });
  fs.writeFileSync(path.join(tmp, 'services', 'a', 'Dockerfile'), 'FROM alpine:3.21\n');
  fs.mkdirSync(path.join(tmp, 'infrastructure'), { recursive: true });
  fs.writeFileSync(path.join(tmp, 'infrastructure', 'docker-compose.yml'),
    'services:\n  mysql:\n    image: mysql:8.0\n');

  const result = scanProject(tmp);
  const refs = result.map((r) => r.ref).sort();
  assert.deepEqual(refs, ['alpine:3.21', 'mysql:8.0']);

  fs.rmSync(tmp, { recursive: true, force: true });
});
```

- [ ] **Step 2: 跑测试确认失败**

```bash
node --test scripts/lib/__tests__/base-images-scanner.test.js
```

Expected: FAIL（`scanProject` 未定义）。

- [ ] **Step 3: 实现 scanProject**

```js
const fs = require('node:fs');
const path = require('node:path');

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
    const lineOffset = 0;
    for (const ref of parseDockerfile(content)) {
      if (!seen.has(ref)) {
        const rel = path.relative(root, f);
        seen.set(ref, { ref, source: `${rel}`, mirror: null });
      }
    }
  }
  for (const f of composeFiles) {
    const content = fs.readFileSync(f, 'utf8');
    for (const ref of parseCompose(content)) {
      if (seen.has(ref)) continue;
      const rel = path.relative(root, f);
      seen.set(ref, { ref, source: `${rel}`, mirror: null });
    }
  }
  return Array.from(seen.values());
}
```

`module.exports` 改为：

```js
module.exports = { parseDockerfile, parseCompose, scanProject };
```

- [ ] **Step 4: 跑测试**

```bash
node --test scripts/lib/__tests__/base-images-scanner.test.js
```

Expected: PASS（6 cases）。

- [ ] **Step 5: 提交**

```bash
git add scripts/lib/Base-Images-Scanner.js scripts/lib/__tests__/base-images-scanner.test.js
git commit -m "feat(scripts): add scanProject walking dockerfiles and compose"
```

---

### Task 4: Writer 合并旧 mirror 字段

**Files:**
- Create: `scripts/lib/Base-Images-Writer.js`
- Create: `scripts/lib/__tests__/base-images-writer.test.js`

- [ ] **Step 1: 写失败测试**

```js
// scripts/lib/__tests__/base-images-writer.test.js
const test = require('node:test');
const assert = require('node:assert/strict');
const { mergeMirror, writeManifest } = require('../Base-Images-Writer.js');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');

test('mergeMirror keeps existing mirror on known refs', () => {
  const scanned = [
    { ref: 'redis:7-alpine', source: 'x', mirror: null },
    { ref: 'mysql:8.0', source: 'y', mirror: null },
  ];
  const old = { images: [{ ref: 'redis:7-alpine', source: 'old', mirror: 'docker.xunyuan.cn' }] };
  const merged = mergeMirror(scanned, old);
  const redis = merged.find((i) => i.ref === 'redis:7-alpine');
  assert.equal(redis.mirror, 'docker.xunyuan.cn');
  const mysql = merged.find((i) => i.ref === 'mysql:8.0');
  assert.equal(mysql.mirror, null);
});

test('writeManifest writes JSON with version + generatedAt', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'imgwrite-'));
  const out = path.join(tmp, 'base-images.json');
  writeManifest(out, [{ ref: 'alpine:3.21', source: 'a', mirror: null }]);
  const data = JSON.parse(fs.readFileSync(out, 'utf8'));
  assert.equal(data.version, 1);
  assert.ok(data.generatedAt);
  assert.equal(data.images.length, 1);
  fs.rmSync(tmp, { recursive: true, force: true });
});
```

- [ ] **Step 2: 跑测试确认失败**

```bash
node --test scripts/lib/__tests__/base-images-writer.test.js
```

Expected: FAIL（模块不存在）。

- [ ] **Step 3: 实现 writer**

```js
// scripts/lib/Base-Images-Writer.js
'use strict';

const fs = require('node:fs');
const path = require('node:path');

const VERSION = 1;

function mergeMirror(scanned, old) {
  const oldByRef = new Map();
  if (old && Array.isArray(old.images)) {
    for (const img of old.images) {
      if (img && img.ref) oldByRef.set(img.ref, img.mirror || null);
    }
  }
  return scanned.map((img) => {
    const oldMirror = oldByRef.get(img.ref);
    return {
      ref: img.ref,
      source: img.source,
      mirror: img.mirror != null ? img.mirror : (oldMirror != null ? oldMirror : null),
    };
  });
}

function writeManifest(outPath, images) {
  const data = {
    version: VERSION,
    generatedAt: new Date().toISOString(),
    images,
  };
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, JSON.stringify(data, null, 2) + '\n');
}

module.exports = { mergeMirror, writeManifest };
```

- [ ] **Step 4: 跑测试**

```bash
node --test scripts/lib/__tests__/base-images-writer.test.js
```

Expected: PASS（2 cases）。

- [ ] **Step 5: 提交**

```bash
git add scripts/lib/Base-Images-Writer.js scripts/lib/__tests__/base-images-writer.test.js
git commit -m "feat(scripts): add base-images writer with mirror merge"
```

---

### Task 5: CLI 入口 base-images-sync.js

**Files:**
- Create: `scripts/base-images-sync.js`

- [ ] **Step 1: 实现 CLI**

```js
#!/usr/bin/env node
// scripts/base-images-sync.js
// 扫描源码中的基础镜像，合并 mirror 字段，写入 infrastructure/base-images.json
//
// 用法：
//   node scripts/base-images-sync.js
//   node scripts/base-images-sync.js --check
//   node scripts/base-images-sync.js --set-mirror <ref> <mirror>

const fs = require('node:fs');
const path = require('node:path');
const { ROOT, log } = require('./lib/utils.js');
const { scanProject } = require('./lib/Base-Images-Scanner.js');
const { mergeMirror, writeManifest } = require('./lib/Base-Images-Writer.js');

const MANIFEST = path.join(ROOT, 'infrastructure', 'base-images.json');

function parseArgs(argv) {
  const out = { command: null };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--check') out.command = 'check';
    else if (a === '--set-mirror') {
      out.command = 'set-mirror';
      out.ref = argv[++i];
      out.mirror = argv[++i];
    }
  }
  return out;
}

function readOldManifest() {
  if (!fs.existsSync(MANIFEST)) return { images: [] };
  try { return JSON.parse(fs.readFileSync(MANIFEST, 'utf8')); }
  catch { return { images: [] }; }
}

function cmdSync(checkOnly) {
  const scanned = scanProject(ROOT);
  const old = readOldManifest();
  const merged = mergeMirror(scanned, old);
  merged.sort((a, b) => a.ref.localeCompare(b.ref));
  if (checkOnly) {
    const oldJson = JSON.stringify((old.images || []).map((i) => i.ref).sort());
    const newJson = JSON.stringify(merged.map((i) => i.ref).sort());
    if (oldJson === newJson) {
      log.green('[sync] no changes');
    } else {
      log.yellow('[sync] drift detected:');
      log.plain(`  old: ${oldJson}`);
      log.plain(`  new: ${newJson}`);
      process.exit(2);
    }
    return;
  }
  writeManifest(MANIFEST, merged);
  log.green(`[sync] wrote ${merged.length} images to ${MANIFEST}`);
}

function cmdSetMirror(ref, mirror) {
  const old = readOldManifest();
  const img = (old.images || []).find((i) => i.ref === ref);
  if (!img) {
    log.red(`[sync] unknown ref: ${ref}`);
    process.exit(1);
  }
  img.mirror = mirror || null;
  fs.writeFileSync(MANIFEST, JSON.stringify(old, null, 2) + '\n');
  log.green(`[sync] ${ref} mirror=${mirror || '(cleared)'}`);
}

function main() {
  const args = parseArgs(process.argv);
  if (args.command === 'set-mirror') {
    if (!args.ref || args.mirror === undefined) {
      log.red('usage: --set-mirror <ref> <mirror>');
      process.exit(1);
    }
    cmdSetMirror(args.ref, args.mirror);
    return;
  }
  cmdSync(args.command === 'check');
}

main();
```

- [ ] **Step 2: 跑一次生成初始清单**

```bash
node scripts/base-images-sync.js
```

Expected: 输出 `[sync] wrote N images to ...\infrastructure\base-images.json`。

- [ ] **Step 3: 校验文件存在且格式正确**

```bash
Get-Content infrastructure\base-images.json | Select-Object -First 5
```

Expected: 包含 `"version": 1` 和 `"images"` 数组。

- [ ] **Step 4: 跑 --check**

```bash
node scripts/base-images-sync.js --check
```

Expected: `[sync] no changes`。

- [ ] **Step 5: 提交**

```bash
git add scripts/base-images-sync.js infrastructure/base-images.json
git commit -m "feat(scripts): add base-images-sync CLI + initial manifest"
```

---

### Task 6: 修改 prePull 读取 JSON + mirror 处理

**Files:**
- Modify: `scripts/lib/Docker-Helper.js`

- [ ] **Step 1: 替换 PRE_PULL_IMAGES 与 prePull**

删除 `PRE_PULL_IMAGES` 常量与原 `prePull()`，替换为：

```js
const MANIFEST_PATH = path.join(ROOT, 'infrastructure', 'base-images.json');

function loadImageManifest() {
  if (!fs.existsSync(MANIFEST_PATH)) {
    log.yellow(`[pull] manifest not found: ${MANIFEST_PATH}, run: node scripts/base-images-sync.js`);
    return [];
  }
  try {
    const data = JSON.parse(fs.readFileSync(MANIFEST_PATH, 'utf8'));
    return Array.isArray(data.images) ? data.images : [];
  } catch (e) {
    log.yellow(`[pull] manifest parse failed: ${e.message}`);
    return [];
  }
}

async function pullOne(ref, mirror) {
  if (mirror) {
    const mirrorRef = `${mirror.replace(/\/+$/, '')}/${ref}`;
    log.cyan(`[pull] ${mirrorRef} (mirror of ${ref})`);
    const mr = spawnSync('docker', ['pull', mirrorRef], { stdio: 'inherit' });
    if (mr.status === 0) {
      const tr = spawnSync('docker', ['tag', mirrorRef, ref], { stdio: 'inherit' });
      if (tr.status === 0) {
        log.green(`[pull] tagged ${mirrorRef} -> ${ref}`);
        return true;
      }
      log.yellow(`[pull] tag failed, image usable as ${mirrorRef}`);
      return true;
    }
    log.yellow(`[pull] mirror failed, fallback to ${ref}`);
  }
  log.cyan(`[pull] ${ref}`);
  const res = spawnSync('docker', ['pull', ref], { stdio: 'inherit' });
  if (res.status !== 0) {
    log.yellow(`[pull] WARNING: failed to pull ${ref} (will retry during build)`);
    return false;
  }
  return true;
}

async function prePull() {
  const images = loadImageManifest();
  if (images.length === 0) return;
  for (const img of images) {
    try { await pullOne(img.ref, img.mirror); }
    catch (e) { log.yellow(`[pull] ${img.ref} error: ${e.message}`); }
  }
}
```

- [ ] **Step 2: 同步一次**

```bash
node scripts/base-images-sync.js
```

Expected: `[sync] wrote N images`。

- [ ] **Step 3: 跑一次 prePull 验证 JSON 路径**

不实际拉镜像（依赖网络），只验证读 JSON 不报错：

```bash
node -e "const d = require('./scripts/lib/Docker-Helper.js'); console.log('manifest images:', d.prePull.toString().slice(0, 200))"
```

Expected: 输出包含 `loadImageManifest`。

- [ ] **Step 4: 提交**

```bash
git add scripts/lib/Docker-Helper.js
git commit -m "refactor(scripts): prePull reads base-images.json with mirror fallback"
```

---

### Task 7: Skill 注册

**Files:**
- Create: `.trae/skills/coder-base-images-sync/SKILL.md`
- Modify: `.trae/rules/project_rules.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: 创建 SKILL.md**

```markdown
---
name: coder-base-images-sync
description: 同步项目中的基础 Docker 镜像到 infrastructure/base-images.json，供 test-orchestrator 预拉取使用。
---

# coder-base-images-sync

## 何时使用

- 修改了 `services/*/Dockerfile*`
- 修改了 `infrastructure/docker-compose*.yml`
- 需要为某个镜像配置 mirror（如 `docker.xunyuan.cn`）

## 命令

```bash
node scripts/base-images-sync.js              # 扫描并写入
node scripts/base-images-sync.js --check      # 仅 diff，不写
node scripts/base-images-sync.js --set-mirror redis:7-alpine docker.xunyuan.cn
```

## 输出

`infrastructure/base-images.json`：

```json
{
  "version": 1,
  "generatedAt": "ISO-8601",
  "images": [{ "ref": "...", "source": "path:line", "mirror": "..." }]
}
```

## 行为

- `mirror` 字段拉取成功后会自动 `docker tag <mirror>/<ref> <ref>`，本地使用仍是原 tag
- mirror 拉取失败自动回源拉取
- 不删除已不在源码中出现的镜像（v1 策略）

## 不做的事

- ❌ 不修改业务源码
- ❌ 不验证镜像在 registry 中存在
- ❌ 不删 JSON 中已有但源码已不引用的镜像
```

- [ ] **Step 2: 修改 project_rules.md**

在「路径 → Skill 自动匹配」表新增两行（在「服务规范检查」行之前）：

```
| `services/*/Dockerfile*`                | `coder-base-images-sync`       | 改完保存  |
| `infrastructure/docker-compose*.yml`   | `coder-base-images-sync`       | 改完保存  |
```

- [ ] **Step 3: 修改 AGENTS.md Skill 索引**

在 `coder-run-tests` 行后新增：

```
| `coder-base-images-sync` | base images、镜像清单 | node scripts/base-images-sync.js |
```

- [ ] **Step 4: 提交**

```bash
git add .trae/skills/coder-base-images-sync/SKILL.md .trae/rules/project_rules.md AGENTS.md
git commit -m "chore(ai): register coder-base-images-sync skill"
```

---

### Task 8: 端到端验证

**Files:** 无（仅验证）

- [ ] **Step 1: 重新生成 manifest**

```bash
node scripts/base-images-sync.js --check
```

Expected: `[sync] no changes`。

- [ ] **Step 2: 手动添加一个 mirror 并验证 tag**

```bash
node scripts/base-images-sync.js --set-mirror alpine:3.21 docker.xunyuan.cn
Get-Content infrastructure\base-images.json
```

Expected: `alpine:3.21` 的 `mirror` 字段变为 `docker.xunyuan.cn`。

- [ ] **Step 3: 跑 --set-mirror 清空 mirror**

```bash
node scripts/base-images-sync.js --set-mirror alpine:3.21 ""
Get-Content infrastructure\base-images.json
```

Expected: `mirror` 为 `null`。

- [ ] **Step 4: 跑全部单测**

```bash
node --test scripts/lib/__tests__/base-images-scanner.test.js scripts/lib/__tests__/base-images-writer.test.js
```

Expected: PASS（所有 cases）。

- [ ] **Step 5: 提交（若有变更）**

```bash
git status
# 仅当 base-images.json 有意变更时：
git add infrastructure/base-images.json
git commit -m "chore(scripts): verify base-images-sync end-to-end"
```

---

## Self-Review

- Spec 1: JSON 结构 → Task 4 ✅
- Spec 2: scanner → Task 1/2/3 ✅
- Spec 3: writer + mirror 合并 → Task 4 ✅
- Spec 4: CLI → Task 5 ✅
- Spec 5: prePull 重写 → Task 6 ✅
- Spec 6: Skill 注册 → Task 7 ✅
- Spec 7: 测试 → Task 1/2/3/4 + Task 8 ✅
- Spec 8: 不删镜像 / 不验证镜像 → Task 7 SKILL.md "不做的事" + Task 6 行为 ✅
- 占位符扫描：无 TBD/TODO
- 类型一致：`parseDockerfile` / `parseCompose` / `scanProject` / `mergeMirror` / `writeManifest` / `pullOne` / `prePull` 全程同名