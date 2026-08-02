# cms-sync: skip 列表真正生效 + `protected-managed` 策略

> **状态**: ✅ skip 语义已实现 (commit `f6d8664`); ⏳ protected-managed 已实现 (commit pending)
> **日期**: 2026-08-02
> **影响范围**: `release/dist-tools/lib/{sync,parse-policy}.js` + 测试 + 文档

---

# 第一部分: skip 语义

## 问题

`.upstream-sync.yaml` 的 `skip` 段被**解析**但**未生效**：

```js
// sync.js (修复前)
const managedFiles = expandGlobs(policy.managed, distRoot);
const mergeFiles   = expandGlobs(policy.merge, distRoot);
// policy.skip 完全没读
```

实际效果：不在 `managed` / `merge` glob 里的文件**隐式**不动（与显式 skip 行为相同），但 yaml 里写 `skip: frontend/**` 是**装饰性的**——只能起到"标注"作用，不能真正阻止 sync，也无法在 sync 输出中看出 skip 命中了哪些文件。

## 设计

| 类别 | skip 行为 | 原因 |
|---|---|---|
| `managed` | **无视 skip**（显式优先） | 显式列出 = 强制 sync，不能被通配意外吞掉 |
| `merge` (`three-way-merge`) | **skip 生效** | 防止"忘记下游改了"的意外覆盖 |
| 隐式不动 | 与显式 skip 同效 | 不在 managed/merge/skip 任一列表 = 不动 |

冲突检测：`managed ∩ skip` 文件数 > 0 时输出 `warn`，列出文件路径，便于策略审计。

## 实现

### `sync.js` 关键改动

```js
// 5. Expand globs against dist tree
const skipFiles = new Set(expandGlobs(policy.skip || [], distRoot));
const rawManaged = expandGlobs(policy.managed, distRoot);
const rawMerge   = expandGlobs(policy.merge, distRoot);

// 6. Apply skip: managed always wins over skip (explicit > wildcard).
//    For merge files, skip wins (no accidental force-merge).
const managedFiles = rawManaged;
const mergeFiles   = rawMerge.filter((f) => !skipFiles.has(f));

const overlaps = rawManaged.filter((f) => skipFiles.has(f));
if (overlaps.length > 0) {
  r.warn(`${overlaps.length} file(s) listed in BOTH managed and skip — managed wins:`);
  for (const f of overlaps) r.log(`    ${f}`);
}
```

### 测试

新增 `lib/sync.test.js`，4 个 case：
1. skip 排除 merge 文件（无 managed override）
2. managed wins over skip
3. 既不在任何列表 = 隐式不动
4. 空 skip 列表 = no-op

`npm test` 现跑 9 个 case（原 5 + 新 4），全部通过。

### 文档

`README.md` 表格 + 新增「优先级与 skip 的语义」章节，明确：
- managed > skip
- merge + skip = skip wins
- 既不在任何列表 = 不动
- 冲突 warn

## 不在范围内

- 不改 `parse-policy.js`（skip 解析已正确）
- 不改 `expand-globs.js`（无变化）
- 不发布 npm 包版本（与本次修复独立）

---

# 第二部分: `protected-managed` 策略

> **状态**: ⏳ 已实现，待 commit
> **触发问题**: `infrastructure/docker-compose.yml` 这种双方都可能改、下游可能通过 `include:` 加业务 compose 的文件，无论是 managed 覆盖还是 merge 处理都不是理想方案

## 设计

新策略段 `protected-managed`：dst 不被覆盖，上游版本另存为 `<file>.new.<ext>`。

| 状态 | 行为 |
|------|------|
| 上游包里有，下游没有 | 正常 copy（如首次安装） |
| 上下游内容相同 | unchanged |
| 上下游都存在但内容不同 | **dst 保留**，上游版本另存为 `<file>.new.<ext>` + warn |

## 与其他策略的关系

- 与 `managed` 互斥：同一文件不应同时列入
- 与 `merge` 互斥：从 `mergeFiles` 中排除（否则 merge 循环会先覆盖文件，使 sidecar 看到"identical"误报 unchanged）
- 与 `skip` 同等：被 skip 命中的 protected-managed 文件也被排除

## 实现

### `parse-policy.js` 加 `protected-managed` 段

```js
return {
  managed: result.managed || [],
  merge: result.merge || result['three-way-merge'] || [],
  skip: result.skip || [],
  'protected-managed': result['protected-managed'] || [],
};
```

### `sync.js` 处理流程

```js
// 6. Apply skip: managed wins over skip. protected-managed wins over merge.
const protectedFiles = rawProtected.filter((f) => !skipFiles.has(f));
const protectedSet = new Set(protectedFiles);
const mergeFiles = rawMerge.filter((f) => !skipFiles.has(f) && !protectedSet.has(f));
```

新增步骤 9（在 merge 循环之后）：

```js
for (const rel of protectedFiles) {
  const srcPath = path.join(distRoot, rel);
  const dstPath = path.join(root, rel);

  if (!fs.existsSync(srcPath)) {
    counts.unchanged++;
    r.item('upstream missing — dst preserved', 'dim', rel);
    continue;
  }
  if (!fs.existsSync(dstPath)) {
    // First install: copy as-is, no sidecar suffix.
    ...
  }
  if (fileIdentical(srcPath, dstPath)) { counts.unchanged++; ... }
  // dst exists and differs — write src as <file>.new.<ext>, keep dst.
  const newPath = sidecarPath(dstPath);
  ...
}
```

新增 `sidecarPath(dstPath)`：

```js
const ext = path.extname(dstPath);
const base = dstPath.slice(0, dstPath.length - ext.length);
return base + '.new' + ext;
```

## 测试

`lib/sync.test.js` 加 5 个 case：
1. `sidecarPath` 命名约定（5 种扩展名）
2. `protected-managed: dst missing → create`
3. `protected-managed: dst identical → unchanged`
4. `protected-managed: dst differs → sidecar`
5. `protected-managed: extensionless filename → Makefile.new`

`lib/parse-policy.test.js` 加 1 个 case：
1. `parses protected-managed block`

`npm test` 现跑 16 个 case（原 11 + 新 5），全部通过。

## 文档

`README.md` 表格新增 `protected-managed` 行 + 新增「`protected-managed` 的行为」章节。

## 下游配置示例

```yaml
protected-managed:
  # 主 compose 入口：上游可能新增 include；下游通常会加业务 compose
  # 同步时上游版本另存为 docker-compose.new.yml，开发者手动 diff 合并
  - "infrastructure/docker-compose.yml"
```

## 验证

- `npm test`：16/16 通过（11 + 5 新增）
- project-manager 真实 sync：
  - 下游修改 docker-compose.yml（加 `docker-compose-pm.yml` include）
  - sync 后：`docker-compose.yml` 保留（业务 include 还在），`docker-compose.new.yml` 生成（上游纯版本），warn 正确

## 不在范围内（增量 #2）

- 不发布 npm 包版本（与本次增量独立）
- 不修改 `transform-compose.js` 或 `release.js`（与本仓 release 流程独立）