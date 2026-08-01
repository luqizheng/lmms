# 基镜像源切到 `mcr.microsoft.com` + 修复 `scripts/lib/utils.js` ROOT 默认值

## Status: 🟡 待 review

### 背景

CI 中三个 job 的 `image:` 字段引用 `$BASE_REGISTRY/...`：

- `dotnet_sdk_test` → `zhcoder-docker-registry.com:8000/official/dotnet-sdk:10.0`
- `sdk_publish_dotnet` → 同上
- `e2e_ui_test` → `zhcoder-docker-registry.com:8000/official/playwright:v1.48.0-jammy`

其中 `$BASE_REGISTRY = zhcoder-docker-registry.com:8000/official`。但是：

> **`official` 是 Harbor 的 proxy cache project**（不是普通 push 仓库），Harbor 拒绝客户端 push：`error from registry: denied: can not push artifact to a proxy project: official`。

并且 `official/dotnet-sdk` 与 `official/playwright` 两个仓库**完全空**（tag 列表为 `[]`）。proxy 的上游配置显然没把这两个镜像正确指向 MCR（或 upstream 失效），导致：
1. CI runner 上 `docker pull` 必然 404
2. `images-pull.js` 在本地也拉不到

### 副作用：`scripts/lib/utils.js` ROOT 写死

`scripts/lib/utils.js` 第 5 行：

```js
const ROOT = process.env.CODER_ROOT || 'd:\\projects\\coder-micro-services';
```

默认值是写死的 Windows 路径。Linux / 任何 CI runner 跑 `pnpm run images:sync` 时：
- `scanProject(ROOT)` 在 `d:\projects\...` 下找 Dockerfile / compose / `.gitlab-ci.yml`，找不到，return 空
- `mergeMirror(scanned, old)` 返回空（因为只 map scanned）
- `writeManifest` 写出空 manifest，实际写到 `d:\projects\.../infrastructure/base-images.json`（Linux 上这是个**字面路径**，会被解析成当前目录下的 `d:\projects\...` 子目录），原 manifest 因为路径不对**没被清**——但 sync 写出的"空 manifest"留在仓库根目录的脏目录里污染文件系统。

---

## 1. 现状盘点

### 失败的镜像

| 镜像 | Harbor `official/` 状态 | 上游 |
|---|---|---|
| `dotnet-sdk:10.0` | empty repo (`tags: []`) | `mcr.microsoft.com/dotnet/sdk:10.0` |
| `playwright:v1.48.0-jammy` | empty repo (`tags: []`) | `mcr.microsoft.com/playwright:v1.48.0-jammy` |

Harbor token endpoint 验证：

```bash
$ curl http://zhcoder-docker-registry.com:8000/v2/official/dotnet-sdk/manifests/10.0
HTTP 404 → "resource not found: repo official/dotnet-sdk, tag 10.0 not found"

$ curl http://zhcoder-docker-registry.com:8000/v2/official/dotnet-sdk/tags/list
{"name":"official/dotnet-sdk","tags":[]}
```

`official/` 项目下绝大部分镜像**确实被 proxy 正确缓存**了（rust/node/maven/nginx 都能正常 pull），单是这两个空仓库是 proxy upstream 配置漂移。

### `images-pull.js` 输出

修复前 `node scripts/images-pull.js`：
```
summary: 17/19 succeeded
failed refs:
  - zhcoder-docker-registry.com:8000/official/dotnet-sdk:10.0
  - zhcoder-docker-registry.com:8000/official/playwright:v1.48.0-jammy
```

---

## 2. 范围

### 2.1 在范围

**a) 三个 CI job 的 `image:` 切换到 MCR：**
- `dotnet_sdk_test` / `sdk_publish_dotnet`: `mcr.microsoft.com/dotnet/sdk:10.0`
- `e2e_ui_test`: `mcr.microsoft.com/playwright:v1.48.0-jammy`

**b) `scripts/lib/utils.js` ROOT 默认值修复：**
从写死的 Windows 路径改为：
```js
const ROOT = process.env.CODER_ROOT || path.resolve(__dirname, '..', '..');
```
保留 `CODER_ROOT` 环境变量覆盖能力（兼容特殊场景）。

**c) 重新生成 manifest 并端到端验证：**
- `node scripts/base-images-sync.js` 重新生成 `infrastructure/base-images.json`
- `node scripts/images-pull.js` 验证 **19/19 succeeded**

### 2.2 不在范围

- ❌ **不修 Harbor `official/` proxy 配置** —— 需 admin 权限和上游策略变更，非 code change 范围
- ❌ **不删 `BASE_REGISTRY` 变量** —— 其他 job（`rust_test`、`spec_check`、`e2e_api_test` 等）仍依赖它
- ❌ **不动 `release-push.js`** —— 当时未追踪的发布脚本，不在本 commit 范围（**已于 2026-08 废弃，功能合并进 build-docker.js**）
- ❌ **不动 Harbor `official/dotnet-sdk` / `official/playwright` 的空仓库** —— 留作 proxy 修复后回退缓存位

---

## 3. 设计取舍

### 为什么改 `.gitlab-ci.yml` 而非直接改 `base-images.json`

`scripts/base-images-sync.js` 的逻辑：

```js
scanned = scanProject(ROOT);                  // 扫源码
merged = mergeMirror(scanned, old);            // 只 map scanned，旧 ref 会被丢弃
writeManifest(MANIFEST, merged);               // 写回
```

`merged` 只包含 `scanned` 里的 ref；老 manifest 中**已经不再被源码引用的 ref 会在下次 sync 时被丢弃**。

因此：
- 直接改 `base-images.json` 会**在下次 sync 被覆盖**——治标不治本
- 改 `.gitlab-ci.yml` 才是真源头：sync 之后自动反映

### 为什么选 `mcr.microsoft.com` 而不是改回 DockerHub

`dotnet/sdk` 在 DockerHub 上的官方名是 `microsoft/dotnet/sdk`（带 namespace），而 MCR 上是 `mcr.microsoft.com/dotnet/sdk`。

- MCR 是 Microsoft 官方维护的容器仓库，与 Harbor 内网网络可达
- Microsoft Container Registry（MCR）的镜像 tag 与 GitHub release 严格一致，便于锁定版本

`playwright` 同理：`mcr.microsoft.com/playwright:v1.48.0-jammy` 是 Microsoft 维护的官方版本。

### 修复 `ROOT` 默认值，保留 `CODER_ROOT` 环境变量覆盖

下游 CI 工作流或者开发者在子目录内跑 `node scripts/base-images-sync.js` 时，仍可能希望显式指定 `CODER_ROOT`。保留 `process.env.CODER_ROOT` 的优先级判断逻辑，仅替换 fallback 默认值。

---

## 4. 文件改动清单

| 文件 | 操作 | 说明 |
|---|---|---|
| `.gitlab-ci.yml` | 改 3 行 | 3 个 job 的 `image:` 字段 |
| `scripts/lib/utils.js` | 改 1 行 | ROOT 默认值 |
| `infrastructure/base-images.json` | 自动重生成 | sync 输出，dotnet + playwright 两条 ref 变化 |

---

## 5. 验证

```bash
# 1) 不设任何环境变量直接 sync（证明 utils.js 修复有效）
node scripts/base-images-sync.js
# 期望: [sync] wrote 19 images to .../base-images.json

# 2) 不设任何环境变量直接 pull（全程无 Windows 路径污染）
node scripts/images-pull.js
# 期望: summary: 19/19 succeeded

# 3) manifest 抽查
python3 -c "import json; m=json.load(open('infrastructure/base-images.json')); \
  print([i['ref'] for i in m['images'] if 'dotnet' in i['ref'] or 'playwright' in i['ref']])"
# 期望: ['mcr.microsoft.com/dotnet/sdk:10.0', 'mcr.microsoft.com/playwright:v1.48.0-jammy']
```

预期 `summary: 19/19 succeeded`，`failed refs` 为空。
