# Frontend i18n Namespace 错位修复 Plan

**日期**：2026-07-10
**状态**：✅ 已执行（commit `c2d23b4`，2026-07-30）
**关联**：用户报告 `menus.fileSearchFilename` 显示原始 key；调查发现是全项目级 i18n namespace 错位 bug。

> **执行回填（2026-07-30）**：实际扫描发现仅 10 keys 调用 `menus.*` 但定义在 `buttons:` / `login:`（非 plan 原估 422/363）。其中 1 key（`pureLogin`）同时存在于 `buttons:` / `login:` / `menus:` 三 namespace（`menus:` 是正确归宿，另两处为 stale 重复）。Python 脚本迁移后验证：yaml.safe_load OK、zh-CN / en 各 586 keys 完美对称、0 个 `menus.*` 调用 fallback。详见 commit message 与 `docs-site/decisions/` 归档（pre-v1-todo.md §P2-7）。

---

## 1. 问题

### 1.1 现象
用户访问 `views/file/index.vue` 页面，看到"文件名"输入框前的 label 是原始字符串 `menus.fileSearchFilename` 而不是翻译后的"文件名"。

### 1.2 根因（已定位）
代码中 `t('menus.X')` 调用，但 yaml 中 `X` 实际定义在 `buttons:`（少数在 `login:`）namespace 下，导致 vue-i18n 找不到 key，fallback 返回原始 key 字符串。

**调用方与 yaml namespace 不一致**——这是 2026-07-10 一系列 "B plan i18n-ize" commits 留下的系统性 bug：

| commit | 主题 |
|---|---|
| `b0ff011` | notification-admin |
| `8d1ff41` | notification-inbox |
| `fc0cab5` | system-user |
| `2b83a9c` | account-settings index |
| `0cb50ae` | account-settings-components (Preferences / AccountManagement / SecurityLog) |
| `444fc0a` | account-settings-profile |
| `700c9e8` | system-dept-role |
| `7cf978f` | system-permission-token-client |
| `27a2014` | system-tenant-session-menu |
| `3e846b3` | system-role-form |
| `4fd8051` | config (namespace + item) |
| `2d8188a` | file (index + presign) |
| `383d556` | monitor (retention + operation-log) |
| `7164ca3` | system-form (dept + menu) |
| `ed2faa8` | system-form-api-keys |

每个 commit 的代码用 `t('menus.X')`，但 yaml 把 `X` 加到了 `buttons:` namespace 下。

### 1.3 全项目扫描结果（实测）

```
$ bash /tmp/scan-all-namespace.sh | wc -l
422
$ bash /tmp/scan-all-namespace.sh | awk -F'|' '{print $3}' | sort | uniq -c
    421 buttons
      1 login
```

- **422 处 namespace 错位调用**
- **363 个唯一 key**（421 + 1 + 0 个多文件共用）
- **39 个受影响文件**（`.vue` + `.test.ts`）

完整 list 已生成在 `/tmp/keys-to-move.txt`（363 行）。

### 1.4 当前 git 状态

```
modified:   frontend/locales/en.yaml    (+1)
modified:   frontend/locales/zh-CN.yaml (+1)
```

这 2 行是计划开始前已落地的 `fileSearchFilename` 单点修复（不属于本 plan 范围，保留不 revert）。

---

## 2. 修复方案

### 2.1 选型

| 选项 | 描述 | 取舍 |
|---|---|---|
| A. 改 yaml：把 keys 从 `buttons:`/`login:` 移到 `menus:` | 调用方代码零改动；一次到位；尊重 commit 中开发者"用 menus: namespace"的意图（commit message 都明示了 `t('menus.xxx')`） | yaml 数据迁移有机械工作量，但可控 |
| B. 改 .vue/.test.ts：把 `t('menus.X')` 改成 `t('buttons.X')` | 改 38+ 个文件、422 处调用；yaml 不动 | 工作量大、易漏改；破坏未来 yaml 重构意图 |
| C. 重整 namespace（新建 `views:`/`digua_common:` 等） | 语义最清晰 | 远超 bugfix 范围；属新设计 |

**采用 A**：最小修复 + 一次到位 + 符合 commit 意图。

### 2.2 改动范围

#### 2.2.1 `frontend/locales/zh-CN.yaml`
- 从 `buttons:` namespace（line 1-411）移除 **362 个 keys**
- 从 `login:` namespace 移除 **1 个 key**：`pureBack`
- 把这 363 行全部附加到 `menus:` namespace **末尾**（line 656 后、`status:` 前）

#### 2.2.2 `frontend/locales/en.yaml`
- 同步 zh-CN.yaml 的所有迁移动作

#### 2.2.3 不动的部分
- 已正确在 `menus:` namespace 的 keys（如 `pureHome`、`pureLogin`、本次新增的 `fileSearchFilename` 等）
- `buttons:` namespace 仍保留的"真按钮" key（如 `pureAccountSettings`、`accountCardButtonModify` 这类**没有**任何 `t('menus.X')` 调用的）
- `login:` namespace 下**非** `pureBack` 的 keys
- `.vue`/`.test.ts` 调用方代码（**零改动**）

### 2.3 实施方法

Python 脚本处理（yaml 缩进敏感，手工 edit 易错）：

```python
import re

with open('/home/kudi/low-memory-micro-services/frontend/locales/zh-CN.yaml') as f:
    lines = f.readlines()

with open('/tmp/keys-to-move.txt') as f:
    keys_to_move = set(line.strip() for line in f if line.strip())

moved_lines = []
remaining_lines = []
for line in lines:
    m = re.match(r'^  ([A-Za-z]+):', line)
    if m and m.group(1) in keys_to_move:
        moved_lines.append(line)
    else:
        remaining_lines.append(line)

# 在 menus: 末尾（status: 前一行）插入
result_lines = []
inserted = False
for line in remaining_lines:
    result_lines.append(line)
    if not inserted and line.startswith('status:'):
        insert_at = len(result_lines) - 1
        for m_line in moved_lines:
            result_lines.insert(insert_at, m_line)
            insert_at += 1
        inserted = True

with open('/.../zh-CN.yaml', 'w') as f:
    f.writelines(result_lines)
```

en.yaml 同样处理。

---

## 3. 验证

### 3.1 yaml 结构验证（执行后必做）
1. 363 keys 全部不在 `buttons:`/`login:` namespace 下：
   ```bash
   for key in $(cat /tmp/keys-to-move.txt); do
     actual=$(awk -v k="^  $key:" '/^[a-zA-Z]+:$/{ns=$0} $0 ~ k {print ns; exit}' frontend/locales/zh-CN.yaml)
     [[ "$actual" != *"menus:"* ]] && echo "FAIL: $key 仍在 $actual"
   done
   ```
2. yaml 格式未损坏：
   ```bash
   python3 -c "import yaml; yaml.safe_load(open('frontend/locales/zh-CN.yaml')); print('OK')"
   python3 -c "import yaml; yaml.safe_load(open('frontend/locales/en.yaml')); print('OK')"
   ```
3. `menus:` namespace key 数应从 ~296 增至 ~657：
   ```bash
   sed -n '/^menus:/,/^[a-zA-Z]/p' frontend/locales/zh-CN.yaml | grep -c "^  [A-Za-z]"
   ```

### 3.2 运行时验证（vite dev + 抽样 t() 调用）
启动 `pnpm run dev` 后用 Node 模拟 vue-i18n：
- `t('menus.fileSearchFilename')` 在 `zh` locale → `"文件名"`；`en` locale → `"File Name"`
- 抽样 5-10 个典型 key（如 `menus.fileBulkDelete`、`menus.configDeleted`、`menus.apiKeyCreate`、`menus.sessionRevoke`、`menus.accountCardTitlePassword`）确认双 locale 都正确翻译

### 3.3 单测（`pnpm run test:unit`）
- 跑 `frontend/` 全量 vitest 测试
- 关注 `views/file/index.test.ts`、`views/file/presign.test.ts`、`views/system/tenant/index.test.ts`、`views/system/session/index.test.ts`、`views/system/token-client/api-keys.vue`（**这些 .test.ts 里也有错位调用，迁移 yaml 后这些 test 才能通过**）
- 跑前确认 `.eslintcache` 已清（避免 stale 结果）

### 3.4 类型检查（`pnpm run typecheck`）
- 跑 TypeScript 编译验证

### 3.5 i18n parity 校验
- 项目有 `check:frontend-i18n` 或类似脚本（`grep -c` zh-CN vs en.yaml 顶层 key 数）
- zh-CN.yaml 与 en.yaml 在 `menus:` namespace 下 key 数必须相等

---

## 4. 风险与回滚

### 4.1 风险
- **格式损坏**：脚本逻辑如果写错，会破坏 yaml 结构（已在 plan §2.3 用 Python 限定）
- **遗漏 / 重复**：363 个 keys 中如有重名或前缀冲突，会出错（`/tmp/keys-to-move.txt` 已用 `sort -u` 去重；yaml 中无重名 key，因为 uniq 输出 363 个）
- **影响范围意外扩大**：如果有些 key 实际**应该**留在 `buttons:`（语义正确），错误地移到 `menus:` 会导致其他调用 `t('buttons.X')` 失效
  - **缓解**：迁移后用 `/tmp/scan-all-namespace.sh` 重跑，如果**反向**错位（如 `t('buttons.X')` 但 X 在 menus:）出现 > 0，说明误移——立刻 revert

### 4.2 回滚方案
```bash
git checkout frontend/locales/zh-CN.yaml frontend/locales/en.yaml
```
（保留 `fileSearchFilename` 那 2 行改动，需要从 git stash 恢复或重新 cherry-pick）

更安全：
```bash
# 在 plan §1.4 当前 git state 基础上执行修改
# 如果验证失败：
git checkout HEAD -- frontend/locales/zh-CN.yaml frontend/locales/en.yaml
```

---

## 5. 执行计划（串行）

按用户要求"先 zh-CN.yaml，再 en.yaml"串行：

| 步骤 | 操作 | 期望产物 |
|---|---|---|
| 1 | 启动 subagent 修改 `zh-CN.yaml`（Python 脚本） | `zh-CN.yaml` 改完 |
| 2 | 跑 §3.1 验证脚本 | 0 个 FAIL + yaml.safe_load OK + menus: key 数 ≈ 657 |
| 3 | 启动 subagent 修改 `en.yaml` | `en.yaml` 改完 |
| 4 | 跑 §3.1 验证（针对 en.yaml） | 同上 |
| 5 | 跑 `/tmp/scan-all-namespace.sh`，确认反向错位 = 0 | 0 行输出 |
| 6 | 启动 vite dev + Node 抽样测试 §3.2 | 抽样 5+ key 双 locale 都正确 |
| 7 | 跑 `pnpm run test:unit` | exit 0 |
| 8 | 跑 `pnpm run typecheck` | exit 0 |
| 9 | git diff 复查 + 报告 user | 等 user review |

每步失败 → 立即 stop + 回滚 + 报告。

---

## 6. 不在范围

- 调用方 `.vue`/`.test.ts` 文件修改（保持原状）
- yaml 中其它 namespace 重整（`search:`/`panel:`/`status:`/`tableBar:`/`changePassword:`）
- 新建 namespace（`views:`/`digua_common:` 等）
- i18n 文案 review / 翻译润色
- 把"纯按钮" key 也从 `buttons:` 移到 `menus:`（如果有 `t('menus.X')` 调用的，已经在 363 keys 范围；纯按钮无调用方的不动）

---

## 7. 完成后产出

- 修改后 `frontend/locales/zh-CN.yaml`（1 个 commit）
- 修改后 `frontend/locales/en.yaml`（同 1 个 commit）
- `pnpm run test:unit` exit 0
- `pnpm run typecheck` exit 0
- 抽样验证：所有原本显示原始 key 的页面现在显示翻译后的中文/英文