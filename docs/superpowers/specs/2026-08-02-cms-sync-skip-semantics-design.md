# cms-sync: skip 列表真正生效

> **状态**: ✅ 已实现 (commit pending)
> **日期**: 2026-08-02
> **影响范围**: `release/dist-tools/lib/sync.js` + 测试 + 文档

## 问题

`scripts/release.js` → `release/dist-template/scripts/release.js` → `release/dist-tools/bin/sync.js` (cms-sync) 中，`.upstream-sync.yaml` 的 `skip` 段被**解析**但**未生效**：

```js
// sync.js (修复前)
const managedFiles = expandGlobs(policy.managed, distRoot);
const mergeFiles   = expandGlobs(policy.merge, distRoot);
// policy.skip 完全没读
```

实际效果：不在 `managed` / `merge` glob 里的文件**隐式**不动（与显式 skip 行为相同），但 yaml 里写 `skip: frontend/**` 是**装饰性的**——只能起到"标注"作用，不能真正阻止 sync。

### 触发的现场

`project-manager` 下游仓 1.0.0 → 1.1.0 升级时，把 `frontend/nginx.conf` 和 `frontend/vite.config.ts` 加入 `managed`，但 `skip` 段仍有 `frontend/**`。sync 正确覆盖（因 managed 优先于隐式 skip），但用户看到的"managed 覆盖 skip"实际上是隐式 skip 与 managed 的冲突，**没有任何提示**——下游仓的策略审计困难。

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

## 验证

- `npm test`：9/9 通过
- project-manager 1.1.0 同步：`frontend/nginx.conf` + `frontend/vite.config.ts` 仍可被 managed 强制覆盖，warn 正确显示