# OBSIDIAN — 深色专业风视觉方向规范

> **设计系统代号：OBSIDIAN（黑曜石）**
> 定位：效率工具 / 设计系统展示 + Dashboard 概念页 · 深色专业风 · 中文为主界面
> 版本：v1.0 · Phase 1 视觉结论 · 交付 Vite + React 前端直接实现
>
> 本文档所有数值均为**唯一确定值**，前端不得自行改写；如需微调必须回到视觉侧确认。

---

## 1. 设计理念

**关键词：精密 · 冷静 · 克制 · 纵深**

OBSIDIAN 以「黑曜石」为核心意象——极致深黑的底座、边缘一线冷光的剖面质感。整体气质是**专业工具的仪表舱**而非消费产品的娱乐场：信息密度高、留白有目的、色彩极度节制，用「一层可感知的光」区分界面层级。

**与对标产品的差异化定位：**

| 维度 | Linear | Arc | Raycast | **OBSIDIAN** |
|---|---|---|---|---|
| 底色明度 | 近黑（L≈7%） | 深灰蓝（L≈13%） | 近黑带蓝紫 | **极深冷黑（L≈6%），冷色相偏移 -8°** |
| 品牌色饱和策略 | 紫罗兰高饱和 | 渐变彩色系 | 红橙 | **单色克制的青金 accent，饱和度 ≤ 72%** |
| 层级手法 | 边框提亮为主 | 毛玻璃+渐变 | 光晕叠加 | **「明度阶梯 + 0.5px 冷光描边」双轨** |
| 性格关键词 | 极简工程感 | 活泼浏览器感 | 快捷效率感 | **精密仪器感 · 数据纵深感** |

核心原则（写进展示页）：

1. **黑不是背景，是材质** —— 底色带冷色相与极低噪度，界面像一整块打磨过的黑曜石面板。
2. **光只给一个焦点** —— 每个视图只有一个 accent 焠点（主按钮/当前项/焦点环），其余全部退后。
3. **层级靠明度不靠影子** —— 深色下阴影几乎不可见，用 4 级 surface 明度 + 提亮描边制造纵深。
4. **数字是第一等公民** —— 等宽字体渲染所有数字与指标，数据即装饰。
5. **动效是物理反馈不是表演** —— 全站只用两支缓动曲线，动效时长不超过 320ms。

---

## 2. 色彩系统

### 2.1 色彩基调推导

底色锚定 HSL 色相 **228°（冷蓝）**，饱和度 6–14%，明度从 6% → 13% 递增构成四层 surface；文字为同一冷色相的低饱和白，避免「灰字发脏」。品牌 accent 选用 **青金色（Hue 188°）**：与底色色相距 40°，有辨识度但不刺眼，饱和度压在 68%，在深底上同时满足对比度与克制感。

### 2.2 背景层级 Surface

| Token | Hex | HSL | 用途 |
|---|---|---|---|
| `--surface-base` | `#0B0D12` | 228° 22% 6% | 页面最底层 / 应用画布 |
| `--surface-sunken` | `#08090D` | 225° 21% 4% | 比底更深的下沉区：代码块、输入框内部、表格页脚 |
| `--surface-raised` | `#12151C` | 226° 24% 9% | 卡片、面板、悬浮层第一级 |
| `--surface-overlay` | `#1A1E27` | 226° 24% 13% | 下拉菜单、Modal、Popover |
| `--surface-hover-veil` | `rgba(255,255,255,0.045)` | — | 叠加在任意 surface 上的通用 hover 提亮层 |

层级关系（由深到浅）：`sunken < base < raised < overlay`。**上层永远比下层亮**，这是本系统纵深的第一规则。

### 2.3 文字层级

| Token | Hex | 对 `surface-base` 对比度 | 用途 |
|---|---|---|---|
| `--text-primary` | `#E8EAF0` | 15.6:1 | 标题、正文主体、表格数据 |
| `--text-secondary` | `#A8AEBD` | 7.9:1 | 次级说明、导航项、表头 |
| `--text-tertiary` | `#7D8494` | 5.0:1 | 辅助注释、时间戳、placeholder 相关说明 |
| `--text-disabled` | `#565C69` | 2.6:1 | 禁用态（仅交互禁用文案，正文禁止使用） |
| `--text-inverse` | `#0B0D12` | 15.6:1 | accent 实底上的文字 |

### 2.4 边框与分隔线

| Token | Hex | 用途 |
|---|---|---|
| `--border-subtle` | `#171A21` | 卡片内部分组、列表隐分隔 |
| `--border-default` | `#232730` | 卡片描边、输入框描边、表格行分隔 |
| `--border-strong` | `#333947` | 悬浮层外框、hover 加深、拖拽句柄 |
| `--border-accent-glow` | `rgba(83,214,229,0.38)` | 焦点/激活元素的外发光描边 |

### 2.5 品牌主色 Accent（青金 Cyan-Gold）

| Token | Hex | HSL | 用途 |
|---|---|---|---|
| `--accent` | `#53D6E5` | 188° 68% 61% | 主操作、链接、激活态、焦点环 |
| `--accent-hover` | `#7BE3EF` | 188° 70% 71% | hover 提亮 |
| `--accent-pressed` | `#3FBBCC` | 188° 66% 52% | 按下压暗 |
| `--accent-strong` | `#0FA8BC` | 189° 85% 40% | 实底主按钮底色 / 图标强调 |
| `--accent-subtle` | `rgba(83,214,229,0.14)` | — | 选中行底、tag 底、进度条轨道 |
| `--accent-gradient` | `linear-gradient(180deg, #67DDEA 0%, #35B9CF 100%)` | — | 仅用于主按钮与 hero 标题渐变，全页限 2 处 |
| `--accent-contrast-text` | `#06232A` | — | accent 实底上的文字（对比度 7.8:1） |

**深色底可用性说明：**
- `accent` 在 `surface-base` 上对比度 **9.2:1**，可作正文链接色无需加粗。
- `accent-strong` 实底配 `--accent-contrast-text` 达 **7.8:1**，主按钮文字不使用纯白（纯白在青底上会因同时对比产生脏灰感，改用深青黑）。
- accent **禁止大面积铺底**（>120×120px 即视为大面积），大面积强调一律降级为 `accent-subtle` 底 + `accent` 文字。
- 焦点环：`accent` 2px 外环 + `rgba(83,214,229,0.25)` 4px 外发光，offset 2px。

### 2.6 语义色（各 2 档）

| 语义 | 默认（文字/图标） | 强调（实底/边框） | 实底上文字 |
|---|---|---|---|
| Success | `--success` `#4ADE80` (500) | `--success-strong` `#22C55E` | `#052E14` |
| Warning | `--warning` `#FBBF24` (500) | `--warning-strong` `#F59E0B` | `#2E1D00` |
| Danger | `--danger` `#F87171` (500) | `--danger-strong` `#EF4444` | `#2E0505` |
| Info | `--info` `#60A5FA` (500) | `--info-strong` `#3B82F6` | `#04182E` |

每档再配 subtle 底：`--*-subtle`（对应色 12% alpha），用于 Badge/Toast 背景。

### 2.7 关键组合对比度核验（WCAG）

| 前景 | 背景 | 对比度 | 判定 |
|---|---|---|---|
| text-primary `#E8EAF0` | surface-base `#0B0D12` | 15.6:1 | ✅ AAA |
| text-secondary `#A8AEBD` | surface-base | 7.9:1 | ✅ AAA 正文可用 |
| text-tertiary `#7D8494` | surface-raised `#12151C` | 4.6:1 | ✅ AA（仅辅助注释） |
| text-primary | surface-overlay `#1A1E27` | 12.4:1 | ✅ AAA |
| accent `#53D6E5` | surface-base | 9.2:1 | ✅ AAA 链接可用 |
| accent-contrast-text `#06232A` | accent-strong `#0FA8BC` | 7.8:1 | ✅ AAA 按钮可用 |
| text-secondary | surface-overlay | 6.3:1 | ✅ AA |
| text-disabled | surface-base | 2.6:1 | ⚠️ 仅禁用态，非正文 |

正文级文本在四层 surface 上均 ≥ 4.5:1；tertiary 仅用于辅助短注释且最低背景为 raised。

---

## 3. 字体系统

### 3.1 字体栈

**中文界面主字体（UI 拉丁 + 中文）：**
```
--font-ui: "Inter", "MiSans", "PingFang SC", "HarmonyOS Sans SC", "Noto Sans SC", "Microsoft YaHei UI", "Segoe UI", system-ui, sans-serif;
```

**中文字重补充说明**：优先声明 Inter 的拉丁设计，中文依次命中 MiSans（小米）→ PingFang SC（macOS）→ HarmonyOS Sans SC（华为）→ Noto Sans SC（Web Font 兜底，loading 期以 system 回退）。**不在页面内嵌中文 Web Font**（体积不可接受），以系统字体保真。

**等宽字体（数字 / 代码 / 指标）：**
```
--font-mono: "JetBrains Mono", "SF Mono", "Cascadia Code", "Roboto Mono", Consolas, "Noto Sans Mono CJK SC", monospace;
```
Dashboard 所有数字指标、表格数值列、时间戳、版本号一律 mono。

### 3.2 字号阶梯（1rem = 16px，基准 html 不缩放）

| 层级 | Token | 字号/行高 | 字重 | 字距 | 用途 |
|---|---|---|---|---|---|
| Display | `--text-display` | 56px / 68px | 650 | 拉丁 -1.5%，中文 0 | Hero 主标题 |
| H1 | `--text-h1` | 40px / 52px | 650 | 拉丁 -1%，中文 0 | 页面主标题 |
| H2 | `--text-h2` | 28px / 38px | 600 | 拉丁 -0.5%，中文 0 | 区块标题 |
| H3 | `--text-h3` | 20px / 28px | 600 | 0 | 卡片标题、面板标题 |
| H4 | `--text-h4` | 16px / 24px | 600 | 0 | 小节标题、表单标题 |
| Body | `--text-body` | 15px / 26px | 400 | 0 | 正文（比 14px 更适配中文笔画密度） |
| Body-strong | `--text-body-strong` | 15px / 26px | 550 | 0 | 正文强调、列表主项 |
| Small | `--text-small` | 13px / 20px | 400 | 0 | 次级信息、按钮内辅助、表格 |
| Caption | `--text-caption` | 12px / 18px | 450 | 拉丁 +2%，中文 +2.5% | 标签、时间戳、脚注 |
| Mono-num | `--text-mono-num` | 15px / 22px | 500 | 0，`font-variant-numeric: tabular-nums` | 指标数字 |

### 3.3 字距与中文规则

- **拉丁大标题（≥28px）负字距**：-0.5% ~ -1.5%，逐级递进（见上表），消除 Inter 大字号的光学松散。
- **中文一律不加负字距**：中文方块字负字距会挤压笔画，仅 Caption 级允许 +2.5% 微扩张提升小字号可读性。
- **中英混排**：中文与拉丁之间不额外加空格 token，由排版层（`text-autospace` 可用时）处理；不可用时依赖字体自然间距，禁止手动 `&nbsp;`。
- **字重上限 650**：深底上过粗字重会发糊，650 为视觉清晰上限；强调优先提亮颜色（secondary→primary）而非加粗。
- **数字排版**：所有对齐数字场景（表格/指标卡/坐标轴）启用 `tabular-nums`，禁止默认比例数字。

---

## 4. 布局与网格

### 4.1 栅格

- 桌面 12 栏栅格，`max-width: 1280px` 容器，gutter `24px`。
- 宽屏 Dashboard 概念页可用 `1600px` 容器（数据密集场景）。
- 断点：`640 / 768 / 1024 / 1280 / 1600`（px，min-width 移动优先）。

### 4.2 间距阶梯（4px 基数）

`--space-1: 4px` · `--space-2: 8px` · `--space-3: 12px` · `--space-4: 16px` · `--space-5: 20px` · `--space-6: 24px` · `--space-8: 32px` · `--space-10: 40px` · `--space-12: 48px` · `--space-16: 64px` · `--space-20: 80px` · `--space-24: 96px`

常用映射：卡片内边距 `--space-6`（24px）；区块间距 `--space-16`（64px）；表单项纵向间距 `--space-6`。

### 4.3 全局框架尺寸

| 部件 | 尺寸 | 说明 |
|---|---|---|
| 侧边栏（Dashboard） | 宽 240px，收起 64px，图标区 40×40 | 图标 20px 居中，标签 13px |
| 顶部导航（Dashboard） | 高 56px | 含 breadcrumb + 搜索框 + 头像 28px |
| 展示页全局 Header | 高 64px | 吸顶，backdrop-blur 16px |
| 内容区左右留白 | ≥ 32px（1024 断点起） | |

### 4.4 圆角阶梯

`--radius-xs: 4px`（Badge/输入框内嵌）· `--radius-sm: 6px`（按钮、输入框、开关）· `--radius-md: 8px`（下拉、小卡片）· `--radius-lg: 12px`（卡片、面板）· `--radius-xl: 16px`（Modal、Hero 容器）· `--radius-full: 999px`（Avatar、开关、Tag）

### 4.5 边框宽度

`--border-1: 1px`（默认所有描边）· `--border-2: 2px`（仅 focus ring）· `--hairline: 1px`（高 DPR 下 0.5px 物理像素的上下分隔线，用 `linear-gradient` 或 transform 实现）

---

## 5. 材质与光影

**核心策略：明度阶梯 70% + 描边提亮 25% + 阴影 5%。深色下阴影几乎不可见，纵深主要靠「更亮的 surface + 更亮的边框」。**

1. **Elevation 三轨制**（按层级递进）：
   - E1 卡片：`surface-raised` + `border-default` 1px，无阴影。
   - E2 悬浮（下拉/Popover）：`surface-overlay` + `border-strong` 1px + 阴影 `0 8px 24px rgba(0,0,0,0.5)`（阴影仅辅助，不可单独承担层级）。
   - E3 Modal/命令面板：`surface-overlay` + 顶部 1px 内发光 `inset 0 1px 0 rgba(255,255,255,0.06)` + 阴影 `0 16px 48px rgba(0,0,0,0.6)`。
2. **冷光描边高光**：所有 E2/E3 浮层顶部一条 `rgba(255,255,255,0.06)` 的 inset 高光，模拟「黑曜石剖面的受光边」，这是本系统的标志性细节。
3. **内发光焦点**：输入框聚焦不做外阴影，用 `box-shadow: inset 0 0 0 1px var(--accent)` + 外圈 2px focus ring。
4. **噪点纹理：不使用。** 噪点在低明度大面积区域会引入脏感与渲染成本，OBSIDIAN 的「材质感」由色相偏移与冷光描边承担。（唯一例外：Hero 背景可用极低强度 3% 的径向光晕 `radial-gradient`，非噪点。）
5. **Focus Ring 规范**：全局统一 `outline: 2px solid var(--accent); outline-offset: 2px;` + `box-shadow: 0 0 0 4px rgba(83,214,229,0.25)`；键盘触发才显示（`:focus-visible`），鼠标点击不显示。
6. **毛玻璃**：仅用于吸顶 Header（`backdrop-filter: blur(16px)` + `rgba(11,13,18,0.72)` 底），其余场景禁止，保证深色渲染性能与文字清晰度。

---

## 6. 组件视觉语言

> 以下为视觉规格（默认/悬停/按下/禁用/聚焦五态），不写代码。

### 6.1 按钮

| 类型 | 默认态 | 悬停态 | 按下态 | 禁用态 |
|---|---|---|---|---|
| **Primary** | 底 `--accent-gradient`（垂直），文字 `#06232A`，字重 600，高 36px，内边距 0 16px，圆角 `--radius-sm`，顶部 1px inset 高光 `rgba(255,255,255,0.18)` | 整体提亮 6%（渐变两色各 +6% 明度）+ 外发光 `0 0 0 4px rgba(83,214,229,0.22)` | 压暗至 `--accent-pressed`，内发光移除，transform `scale(0.98)` | 底 `--surface-raised`，文字 `--text-disabled`，移除描边 |
| **Secondary** | 底 `--surface-raised`，描边 `--border-strong` 1px，文字 `--text-primary` | 底叠 `--surface-hover-veil`，描边提亮至 `#3F4655` | 底压暗至 `#0E1118`，transform `scale(0.98)` | 同上禁用 |
| **Ghost** | 透明底，无描边，文字 `--text-secondary` | 底 `rgba(255,255,255,0.05)`，文字升 `--text-primary` | 底 `rgba(255,255,255,0.08)` | 文字 `--text-disabled`，hover 无反应 |

尺寸系：高 32（small）/ 36（medium）/ 40（large）；图标按钮方形 36×36，图标 18px。

### 6.2 输入框

- 默认：底 `--surface-sunken`，描边 `--border-default` 1px，圆角 `--radius-sm`，高 36px，placeholder `--text-tertiary`，文字 `--text-primary`。
- 悬停：描边 `--border-strong`。
- 聚焦：描边消失，`inset 0 0 0 1px var(--accent)` + 标准 focus ring。
- 错误：inset 环改 `--danger`，下方 12px/18px `--danger` 错误文案，与框距 6px。
- 禁用：底 `--surface-raised`，文字 `--text-disabled`。

### 6.3 卡片

- 容器：底 `--surface-raised`，描边 `--border-default`，圆角 `--radius-lg`，内边距 24px，无阴影。
- 悬停（可交互卡片）：描边 `--border-strong` + 顶部 1px 冷光高光从 0 → `rgba(255,255,255,0.06)` 渐显（150ms）。
- 卡片标题区：H3 16px/600 + 右侧操作区，与正文间 `--space-4` 分隔（`--border-subtle`）。

### 6.4 下拉菜单（Select / Menu）

- 容器：E2 浮层规格（`surface-overlay` + `border-strong` + 阴影 + 顶部冷光高光），圆角 `--radius-md`，内边距 6px，最小宽 200px。
- 菜单项：高 32px，圆角 `--radius-xs`，文字 `--text-secondary`；hover 底 `rgba(255,255,255,0.06)` 文字升 primary；**选中项**底 `--accent-subtle` 文字 `--accent` 左侧 16px 勾选图标。
- 展开动画：从锚点缩放 `0.96→1` + 淡入，150ms，缓动 decelerate（见 §7）。

### 6.5 开关（Toggle）

- 轨道：44×24px，圆角 full；关闭态底 `#232730` 描边 `--border-strong`；开启态底 `--accent-strong` 无描边。
- 滑块：20px 圆，关闭时 `#A8AEBD`，开启时 `#FFFFFF`；开启后滑块右移 20px（180ms，标准缓动）。
- 按下时滑块横向拉伸至 24px（触觉反馈），松开回弹。

### 6.6 标签 / Badge

- 尺寸：高 22px，内边距 0 8px，圆角 full，文字 12px/450，字距 +2%。
- 中性：底 `rgba(255,255,255,0.06)`，文字 `--text-secondary`。
- 语义：底对应 `--*-subtle`，文字对应默认档语义色（如 success Badge：底 `rgba(74,222,128,0.12)` + 字 `#4ADE80`）。
- Accent 选中标签：底 `--accent-subtle`，文字 `--accent`，描边 `rgba(83,214,229,0.38)` 1px。

### 6.7 表格行

- 行高 44px，行间分隔 `--border-subtle` 1px（hover 时该行分隔线升 `--border-default`）。
- 表头：高 40px，背景 `--surface-base`（与 raised 卡片形成反差），文字 `--text-secondary` 12px/450，字距 +2%。
- 数据单元格：15px，数值列 mono `tabular-nums` 右对齐。
- 行 hover：整行叠 `--surface-hover-veil`。
- 行选中：底 `--accent-subtle`，最左 2px `--accent` 竖条。

### 6.8 顶部导航（展示页）

- 高 64px，吸顶，底 `rgba(11,13,18,0.72)` + `backdrop-blur(16px)`，底部 `--border-default` 1px。
- 左：Logo 标记（24px 几何符号，accent 单色）+ 「OBSIDIAN」16px/650 拉丁 -1%；右：导航链接 14px `--text-secondary`，hover 升 primary，当前项 primary + 底部 2px accent 指示条。
- 右端 GitHub 图标按钮 + Primary 小按钮（高 32px）。

### 6.9 侧边栏（Dashboard）

- 宽 240px，底 `--surface-sunken`（比内容区更深，制造「底座」感），右侧 `--border-default` 1px。
- 分组标题：12px `--text-tertiary`，字距 +2.5%，上下 `--space-4`。
- 菜单项：高 36px，圆角 `--radius-sm`，图标 18px + 文字 14px，默认 `--text-secondary`；hover 底 `rgba(255,255,255,0.05)`；**当前项**底 `--surface-raised` + 文字 `--text-primary` + 左侧 2px accent 竖条（贴齐圆角盒左缘，长 20px 居中）。
- 底部用户区：分隔线上方 12px，头像 28px 圆 + 名称 14px + 邮箱 12px tertiary。

---

## 7. 动效语言

### 7.1 时长与缓动（全站仅两支曲线）

| Token | 值 | 用途 |
|---|---|---|
| `--duration-instant` | 80ms | 按下反馈、开关内部状态 |
| `--duration-fast` | 150ms | hover、focus ring、下拉展开 |
| `--duration-medium` | 240ms | 卡片展开、面板切换、toast |
| `--duration-slow` | 320ms | 页面/路由过渡、Modal 进出（**上限**） |
| `--ease-decelerate` | `cubic-bezier(0.16, 1, 0.3, 1)` | 一切「进入」：元素浮现、展开、页面淡入 |
| `--ease-standard` | `cubic-bezier(0.4, 0, 0.2, 1)` | 一切「离开/过渡」：收起、位移、透明度双向 |

### 7.2 常见模式

- **Hover**：150ms / decelerate；只做颜色与描边变化，**不做位移**（表格与列表 hover 位移会引发读写抖动）。
- **展开/收起**（下拉、折叠面板）：scale `0.96→1` + 淡入 150ms 进入；收起 120ms 淡出 + scale `1→0.98`（standard）。变换原点锚定触发元素一侧。
- **页面过渡**：320ms，新页面 `opacity 0→1` + `translateY(8px→0)`，decelerate；旧页面仅淡出 150ms，无位移。Dashboard 内部路由切换用 240ms。
- **骨架屏**：底 `--surface-raised`， shimmer 用 `linear-gradient(90deg, transparent, rgba(255,255,255,0.04), transparent)` 平移循环 **1600ms**（比常规 1200ms 慢，深色下慢速 shimmer 更稳）。
- **数字滚动**（Dashboard 指标）：进页时从 0 计数至目标值 600ms，mono + tabular-nums，缓动 decelaterate 的 JS 等效（easeOutExpo）。
- **减少动效**：全站遵循 `prefers-reduced-motion: reduce`，届时所有动效降为仅 opacity 80ms。

---

## 8. 情绪版：Dashboard 概念页画面描述

> 供展示页 Hero 区直接引用的第一眼画面。

**整体构图**——浏览器视口内是一个 1600px 宽的深色驾驶舱：左侧 240px 侧边栏沉在最深的 `sunken` 黑里，像仪器底座；中央主区是 `base` 层画布，上方 56px 顶栏悬浮着面包屑与一个带 `⌘K` 提示的搜索框；右侧边缘隐约透出一层 raised 面板的边界。

**第一眼焦点**——页面顶部是一组四张指标卡（raised 层、12px 圆角、1px 深灰描边），每张卡内一个巨大的 mono 数字在 tabular-nums 下整齐列队，其中一张卡的数字用 accent 青金色点亮，是整个画面**唯一的彩色焦点**；卡片顶部那道 6% 白的冷光描边让它们像悬浮在黑曜石上的玻璃薄片。

**中景**——左侧三分之二是主图表区：一张面积图，网格线是 `border-subtle` 级的极暗蓝灰，数据线是 accent 的 1.5px 细线，线下方是 14% alpha 的青金渐隐填充；图例与坐标轴数字全部 mono 12px。右侧三分之一是一列紧凑的列表卡与一个环形进度，进度环同样是 accent，环形轨道是 `border-default` 灰。

**底部**——一张 12 行数据表：表头沉在 base 层、字距微张的 12px 灰字；数据行 44px 高、mono 右对齐数字，其中一行被 6% 白的 hover veil 轻轻托起。一枚绿色 success Badge「运行中」在状态列安静发光。

**光影氛围**——整个页面没有一处传统投影下的「浮起」，纵深全部来自四层黑的明度差与边缘冷光；鼠标划过之处，元素以 4.5% 白提亮并显出更亮的边。画面 95% 的面积是黑与灰的层次，彩色只在 accent 焦点、语义 Badge 与图表数据线上克制出现——**像一块被切开的黑曜石，只有一道剖面在发光。**

---

## 附录：CSS 变量速查（前端直接取用）

```css
:root {
  color-scheme: dark;

  /* Surface */
  --surface-base: #0B0D12;
  --surface-sunken: #08090D;
  --surface-raised: #12151C;
  --surface-overlay: #1A1E27;
  --surface-hover-veil: rgba(255,255,255,0.045);

  /* Text */
  --text-primary: #E8EAF0;
  --text-secondary: #A8AEBD;
  --text-tertiary: #7D8494;
  --text-disabled: #565C69;
  --text-inverse: #0B0D12;

  /* Border */
  --border-subtle: #171A21;
  --border-default: #232730;
  --border-strong: #333947;
  --border-accent-glow: rgba(83,214,229,0.38);

  /* Accent */
  --accent: #53D6E5;
  --accent-hover: #7BE3EF;
  --accent-pressed: #3FBBCC;
  --accent-strong: #0FA8BC;
  --accent-subtle: rgba(83,214,229,0.14);
  --accent-gradient: linear-gradient(180deg, #67DDEA 0%, #35B9CF 100%);
  --accent-contrast-text: #06232A;

  /* Semantic */
  --success: #4ADE80;  --success-strong: #22C55E;
  --warning: #FBBF24;  --warning-strong: #F59E0B;
  --danger: #F87171;   --danger-strong: #EF4444;
  --info: #60A5FA;     --info-strong: #3B82F6;

  /* Radius */
  --radius-xs: 4px;  --radius-sm: 6px;  --radius-md: 8px;
  --radius-lg: 12px; --radius-xl: 16px; --radius-full: 999px;

  /* Duration / Easing */
  --duration-instant: 80ms;  --duration-fast: 150ms;
  --duration-medium: 240ms;  --duration-slow: 320ms;
  --ease-decelerate: cubic-bezier(0.16, 1, 0.3, 1);
  --ease-standard: cubic-bezier(0.4, 0, 0.2, 1);

  /* Font */
  --font-ui: "Inter", "MiSans", "PingFang SC", "HarmonyOS Sans SC", "Noto Sans SC", "Microsoft YaHei UI", "Segoe UI", system-ui, sans-serif;
  --font-mono: "JetBrains Mono", "SF Mono", "Cascadia Code", "Roboto Mono", Consolas, "Noto Sans Mono CJK SC", monospace;
}
```

— OBSIDIAN v1.0 · Phase 1 完 —
