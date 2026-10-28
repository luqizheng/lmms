# PRISM — 明亮现代 SaaS 浅色视觉方向规范

> **设计系统代号：PRISM（棱镜）**
> 定位：效率工具 / 设计系统展示 + Dashboard 概念页 · 明亮现代 SaaS 浅色风 · 中文为主界面
> 版本：v2.0 · Phase 1R 视觉结论 · 完全替换 v1 OBSIDIAN 深色版 · 交付 Vite + React 前端直接实现
>
> 本文档所有数值均为**唯一确定值**，前端不得自行改写；如需微调必须回到视觉侧确认。
> 骨架继承 v1：字体栈、10 级字阶、4px 间距、圆角、断点、动效曲线与时长全部沿用（见 §9 差异表）。

---

## 1. 设计理念

**关键词：明澈 · 纸感 · 精确 · 一线钴蓝**

PRISM 以「棱镜」为核心意象——白光进入，只折射出一条精确的钴蓝。整体气质是**打磨过的现代 SaaS 工作台**：暖灰纸感的画布、大而有目的的留白、极轻的阴影，品牌色像一条导航线贯穿焦点与行动点，而非满屏铺色。它不追求「白纸感」，记忆点是「**三层精心调过的暖灰 + 一条钴蓝 + 少量墨色块**」的三件套。

**与对标产品的差异化定位：**

| 维度 | Vercel | Linear light | Notion | **PRISM** |
|---|---|---|---|---|
| 底色策略 | 纯白 + 纯中性灰 | 冷白 + 冷灰 | 暖白纸感、灰阶偏平 | **暖灰阶三层递进（Hue 45° 低饱和），灰阶有雕刻感** |
| 品牌色策略 | 近黑 + 蓝色链接 | 紫罗兰 | 基本无彩色 | **单一钴蓝 #2E5BFF 全链，配墨色块点缀** |
| 层级手法 | 细边框为主 | 边框 + 轻阴影 | 纯灰阶几乎无影 | **「白卡 + 1px 暖灰边 + 4% 级阴影」三件套** |
| 性格关键词 | 工程极简 | 精致效率 | 文档纸感 | **仪器级明澈 · 印刷品质感** |

核心原则（写进展示页）：

1. **白是纸，灰是墨阶** —— 三层暖灰（base / sunken / 边框灰）精确分层，层级一眼可读，拒绝死白。
2. **一条钴蓝贯穿** —— accent 只承担三件事：主行动、当前态、焦点。每屏一条蓝线，多则降级为灰。
3. **纵深交给阴影，但只给 5%** —— 浅色下阴影回归为主手段，全部压在 4%–14% 不透明度的暖黑上，轻而不脏。
4. **墨色块是标点，不是段落** —— 近黑 ink 面板只出现在页脚、CTA 横幅与键帽，像印刷品上的黑体标点。
5. **动效是物理反馈不是表演** —— 沿用 v1：全站两支缓动曲线，时长不超过 320ms。

---

## 2. 色彩系统

### 2.1 色彩基调推导

灰阶锚定 HSL 色相 **45°（暖）**、饱和度 6–10%，从 94% → 100% 明度构成 surface 阶梯——比纯中性灰多一分「纸」的温度，比 Notion 的暖白更克制；文字用同一暖底的近黑，避免「灰字发冷」。品牌 accent 选用**钴蓝-ultramarine（#2E5BFF，H≈225° S≈100% L≈59%）**：饱和度拉满但明度压住，在白底上文字对比度 5.2:1、实底配白字 5.2:1，同时通过「加深文字档 --accent-deep」保证 tint 上可读——既不是 Vercel 的工具蓝链接，也不是 Linear 的紫，是更饱和、更「印刷油墨」的钴蓝。

### 2.2 背景层级 Surface

| Token | Hex | HSL | 用途 |
|---|---|---|---|
| `--surface-sunken` | `#F0F0EC` | 45° 9% 94% | 下沉区：输入井（侧栏内搜索）、代码块、表头带、键帽底 |
| `--surface-base` | `#F7F7F4` | 45° 7% 96% | 页面画布（最底层） |
| `--surface-raised` | `#FFFFFF` | — | 卡片、面板（白卡） |
| `--surface-overlay` | `#FFFFFF` | — | 下拉菜单、Modal、Popover（与 raised 同为纯白，**有意为之**：浮层靠 shadow-md/lg + border-strong 拉开层级，不再叠灰） |
| `--surface-hover-veil` | `rgba(23,24,28,0.04)` | — | 通用 hover「加灰」叠层（浅色下 hover 是压暗不是提亮） |
| `--surface-active-veil` | `rgba(23,24,28,0.07)` | — | 按下 / 选中叠加层 |
| `--surface-ink` | `#17181C` | 231° 10% 10% | 墨色面板：页脚、CTA 横幅、键帽字（PRISM 记忆点，全页占比 <10%） |

层级关系（由深到浅）：`ink ≪ sunken < base < raised = overlay(白)`。**内容永远比画布白，井永远比画布灰。**

### 2.3 文字层级

| Token | Hex | 对 `#FFFFFF` / 对 `surface-base` 对比度 | 用途 |
|---|---|---|---|
| `--text-primary` | `#17181C` | 17.9:1 / 16.8:1 | 标题、正文主体、表格数据 |
| `--text-secondary` | `#52535B` | 7.4:1 / 6.9:1 | 次级说明、导航项、表头 |
| `--text-tertiary` | `#6A6B73` | 5.3:1 / 5.0:1 | 辅助注释、时间戳、坐标轴 |
| `--text-disabled` | `#9C9DA5` | 2.7:1 / 2.5:1 | 禁用态（仅交互禁用文案，正文禁止） |
| `--text-inverse` | `#FFFFFF` | 17.9:1（对 ink） | 墨色面板与 accent 实底上的文字 |

### 2.4 边框与分隔线

| Token | Hex | 用途 |
|---|---|---|
| `--border-subtle` | `#EDEDE9` | 卡片内部分组、列表隐分隔 |
| `--border-default` | `#E3E3DE` | 卡片描边、输入框描边、表格行分隔 |
| `--border-strong` | `#D4D4CE` | 悬浮层外框、hover 加深、控件可交互边界 |
| `--border-data` | `#8A8B93` | 需要 3:1 非文字对比度的场景：图表轴线、滑轨、分隔强化线（对白 3.4:1） |
| `--border-accent-glow` | `rgba(46,91,255,0.30)` | 焦点/激活元素的外发光描边 |

### 2.5 品牌主色 Accent（钴蓝 Cobalt）

| Token | Hex | 用途 |
|---|---|---|
| `--accent` | `#2E5BFF` | 主操作、链接、激活态、焦点环（对白 5.2:1，可作正文链接） |
| `--accent-hover` | `#3D67FF` | hover 档（对白 4.6:1，仍可作文字） |
| `--accent-pressed` | `#1F44D6` | 按下压暗档（对白 7.4:1） |
| `--accent-deep` | `#2447CC` | **tint 上的文字专用**：选中行/侧栏当前项/Badge 文字（对 `#EBF0FF` 6.5:1） |
| `--accent-subtle` | `#EBF0FF` | 选中底、Badge 底、进度轨道（`--accent` 文字在其上 4.6:1） |
| `--accent-wash` | `rgba(46,91,255,0.08)` | 图表填充、hover 淡染 |
| `--accent-gradient` | `linear-gradient(180deg, #2E5BFF 0%, #2750E6 100%)` | 主按钮与 hero 标题渐变，全页限 2 处 |
| `--accent-contrast-text` | `#FFFFFF` | accent 实底上的文字（对 `#2E5BFF` 5.2:1，对渐变两端 5.2/6.2:1） |

**浅色底可用性说明：**
- `accent` 在 base 上 **4.8:1**、在白卡上 **5.2:1**、在 sunken 上 **4.5:1**，链接与激活文字全场景达标。
- **在 tint（`--accent-subtle`）上一律用 `--accent-deep` 做文字**（6.5:1）；`--accent` 仅作 tint 上的图标与短标签（≥3:1）。
- accent 实底永远配白字（≥5.2:1），**禁止在 accent 实底上放 secondary 灰字**。
- accent 禁止大面积铺底（>120×120px 降级为 `accent-subtle`）——大蓝块在浅色界面会产生视觉噪音。
- 焦点环：2px `--accent` 外环 + `0 0 0 4px rgba(46,91,255,0.18)` 光晕（浅色下光晕 alpha 从 v1 的 25% 降至 18%，防止白底「发脏」），offset 2px。

### 2.6 语义色（各 2 档 + subtle 底）

| 语义 | 默认档（文字/图标，对白 ≥4.5） | 强调档（实底，配白字 ≥4.5） | subtle 底 | subtle 底上文字（用强调档） |
|---|---|---|---|---|
| Success | `--success` `#15803D`（5.0:1） | `--success-strong` `#166534`（白字 7.1:1） | `#ECF8F0` | `#166534`（6.5:1） |
| Warning | `--warning` `#B45309`（5.0:1） | `--warning-strong` `#92400E`（白字 7.1:1） | `#FCF3E3` | `#92400E`（6.4:1） |
| Danger | `--danger` `#DC2626`（4.8:1） | `--danger-strong` `#B91C1C`（白字 6.5:1） | `#FDEDED` | `#B91C1C`（5.7:1） |
| Info | `--info` `#0F766E`（5.5:1，青绿，与钴蓝 accent 明确区分） | `--info-strong` `#115E59`（白字 7.6:1） | `#E6F4F2` | `#115E59`（6.8:1） |

规则：**subtle 底上的正文文字一律用强调档（更深），图标可用默认档（≥3:1）**；`--warning-vivid #D97706` 仅允许做装饰性圆点/图标，禁止做文字。

### 2.7 关键组合对比度核验（WCAG）

| 前景 | 背景 | 对比度 | 判定 |
|---|---|---|---|
| text-primary `#17181C` | surface-raised `#FFFFFF` | 17.9:1 | ✅ AAA |
| text-primary | surface-base `#F7F7F4` | 16.8:1 | ✅ AAA |
| text-primary | accent-subtle `#EBF0FF`（选中行） | 15.8:1 | ✅ AAA |
| text-secondary `#52535B` | surface-raised | 7.4:1 | ✅ AAA 正文可用 |
| text-secondary | surface-base | 6.9:1 | ✅ AAA |
| text-secondary | surface-sunken `#F0F0EC` | 6.5:1 | ✅ AA |
| text-tertiary `#6A6B73` | surface-sunken | 4.7:1 | ✅ AA（全场景最低仍达标） |
| accent `#2E5BFF`（链接） | surface-base | 4.8:1 | ✅ AA 正文链接 |
| accent `#2E5BFF` | surface-raised | 5.2:1 | ✅ AA |
| accent-deep `#2447CC` | accent-subtle `#EBF0FF` | 6.5:1 | ✅ AA tint 上文字 |
| `#FFFFFF` | accent `#2E5BFF`（主按钮实底） | 5.2:1 | ✅ AA 按钮文字 |
| `#FFFFFF` | accent-gradient 两端 `#2E5BFF`/`#2750E6` | 5.2 / 6.2:1 | ✅ AA |
| `#FFFFFF` | accent-hover 渐变顶端 `#3D67FF` | 4.6:1 | ✅ AA（hover 态下限仍达标） |
| `#FFFFFF` | surface-ink `#17181C` | 17.9:1 | ✅ AAA |
| text-disabled `#9C9DA5` | surface-raised | 2.7:1 | ⚠️ 仅禁用态，非正文 |

正文级文本在全部 surface 上 ≥4.5:1；主按钮在默认/hover/pressed 全状态 ≥4.5:1。

---

## 3. 字体系统（骨架沿用 v1，无变化）

### 3.1 字体栈

**中文界面主字体（UI 拉丁 + 中文）：**
```
--font-ui: "Inter", "MiSans", "PingFang SC", "HarmonyOS Sans SC", "Noto Sans SC", "Microsoft YaHei UI", "Segoe UI", system-ui, sans-serif;
```

中文依次命中 MiSans → PingFang SC（macOS）→ HarmonyOS Sans SC（华为）→ Noto Sans SC（兜底）。不内嵌中文 Web Font。

**等宽字体（数字 / 代码 / 指标）：**
```
--font-mono: "JetBrains Mono", "SF Mono", "Cascadia Code", "Roboto Mono", Consolas, "Noto Sans Mono CJK SC", monospace;
```
Dashboard 所有数字指标、表格数值列、时间戳、版本号一律 mono。

### 3.2 字号阶梯（1rem = 16px，与 v1 完全一致）

| 层级 | Token | 字号/行高 | 字重 | 字距 | 用途 |
|---|---|---|---|---|---|
| Display | `--text-display` | 56px / 68px | 650 | 拉丁 -1.5%，中文 0 | Hero 主标题 |
| H1 | `--text-h1` | 40px / 52px | 650 | 拉丁 -1%，中文 0 | 页面主标题 |
| H2 | `--text-h2` | 28px / 38px | 600 | 拉丁 -0.5%，中文 0 | 区块标题 |
| H3 | `--text-h3` | 20px / 28px | 600 | 0 | 卡片标题、面板标题 |
| H4 | `--text-h4` | 16px / 24px | 600 | 0 | 小节标题、表单标题 |
| Body | `--text-body` | 15px / 26px | 400 | 0 | 正文 |
| Body-strong | `--text-body-strong` | 15px / 26px | 550 | 0 | 正文强调、列表主项 |
| Small | `--text-small` | 13px / 20px | 400 | 0 | 次级信息、按钮内辅助、表格 |
| Caption | `--text-caption` | 12px / 18px | 450 | 拉丁 +2%，中文 +2.5% | 标签、时间戳、脚注 |
| Mono-num | `--text-mono-num` | 15px / 22px | 500 | 0，`font-variant-numeric: tabular-nums` | 指标数字 |

### 3.3 字距与中文规则（沿用 v1）

- 拉丁大标题（≥28px）负字距 -0.5% ~ -1.5% 逐级递进；**中文一律不加负字距**，Caption 级 +2.5% 微扩张。
- 中英混排不手动插空格；字重上限 650（浅色下同样适用，粗字重在白底上会糊）；强调优先提色（secondary→primary）而非加粗。
- 所有对齐数字场景启用 `tabular-nums`。

---

## 4. 布局与网格（骨架沿用 v1，无变化）

### 4.1 栅格

- 桌面 12 栏，`max-width: 1280px` 容器，gutter `24px`；Dashboard 概念页 `1600px`。
- 断点：`640 / 768 / 1024 / 1280 / 1600`（min-width 移动优先）。

### 4.2 间距阶梯（4px 基数）

`--space-1: 4px` · `--space-2: 8px` · `--space-3: 12px` · `--space-4: 16px` · `--space-5: 20px` · `--space-6: 24px` · `--space-8: 32px` · `--space-10: 40px` · `--space-12: 48px` · `--space-16: 64px` · `--space-20: 80px` · `--space-24: 96px`

卡片内边距 24px；区块间距 64px；表单项纵向 24px。

### 4.3 全局框架尺寸

| 部件 | 尺寸 | 说明 |
|---|---|---|
| 侧边栏（Dashboard） | 宽 240px，收起 64px，图标区 40×40 | 图标 20px 居中，标签 13px |
| 顶部导航（Dashboard） | 高 56px | breadcrumb + 搜索框（⌘K）+ 头像 28px |
| 展示页全局 Header | 高 64px | 吸顶毛玻璃 |
| 内容区左右留白 | ≥ 32px（1024 断点起） | |

### 4.4 圆角阶梯（沿用）

`--radius-xs: 4px` · `--radius-sm: 6px` · `--radius-md: 8px` · `--radius-lg: 12px` · `--radius-xl: 16px` · `--radius-full: 999px`

### 4.5 边框宽度（沿用）

`--border-1: 1px`（默认）· `--border-2: 2px`（仅 focus ring）· `--hairline: 1px`（高 DPR 0.5px 分隔线）

---

## 5. 材质与光影（浅色重定义）

**核心策略：浅色下阴影回归为主要纵深手段（暖黑低透明度三档）+ 暖灰描边辅助 + hover 一律「加灰」。**

1. **阴影阶梯（全部基于暖黑 `rgba(23,24,28,…)`，不用纯黑不用蓝影污染环境）：**

| Token | 值 | 用途 |
|---|---|---|
| `--shadow-xs` | `0 1px 2px rgba(23,24,28,0.04)` | 静置白卡（E1） |
| `--shadow-sm` | `0 1px 2px rgba(23,24,28,0.05), 0 2px 6px rgba(23,24,28,0.05)` | 卡片 hover、吸顶 Header |
| `--shadow-md` | `0 2px 6px rgba(23,24,28,0.06), 0 12px 24px rgba(23,24,28,0.09)` | 下拉/Popover（E2） |
| `--shadow-lg` | `0 4px 12px rgba(23,24,28,0.08), 0 24px 48px rgba(23,24,28,0.14)` | Modal/命令面板（E3） |
| `--shadow-accent` | `0 1px 2px rgba(30,64,175,0.24), 0 4px 12px rgba(46,91,255,0.22)` | 主按钮专属钴蓝影 |
| `--shadow-accent-lg` | `0 2px 4px rgba(30,64,175,0.28), 0 8px 20px rgba(46,91,255,0.28)` | 主按钮 hover |

2. **Elevation 三轨制**：E1 白卡 = `surface-raised` + `border-default` + `shadow-xs`；E2 浮层 = 白 + `border-strong` + `shadow-md`；E3 Modal = 白 + `border-strong` + `shadow-lg` + 顶部 1px 内高光 `inset 0 1px 0 rgba(255,255,255,0.9)`（浅色下改为强化顶部受光边）。墨色面板（ink）**永远平面无阴影**。
3. **Hover 策略（浅色核心差异）**：白底上 hover「加灰」不提亮——通用 `--surface-hover-veil`（4% 暖黑），按下/选中 7%。可交互卡片 hover = 边框升 strong + `shadow-xs→sm`，**不做位移**。
4. **噪点纹理：不使用**（两版一致）。Hero 背景允许一个 6% 透明度的钴蓝径向光晕 `radial-gradient`。
5. **Focus Ring**：全局 `outline: 2px solid var(--accent); outline-offset: 2px;` + `box-shadow: 0 0 0 4px rgba(46,91,255,0.18)`；仅 `:focus-visible` 显示。**ink 墨色面板例外**：光晕加深为 `--focus-glow-ink: rgba(46,91,255,0.32)`，保证暗底可见。
6. **毛玻璃：保留但收缩到两处**——展示页吸顶 Header（`rgba(247,247,244,0.82)` + `backdrop-filter: blur(12px)`）与 Dashboard 顶栏（同规格）。下拉/Modal 一律实底白，保证文字锐度。
7. **键帽（KBD）材质**：底 `--surface-base`、描边 `--border-strong`、`box-shadow: inset 0 -1px 0 #D4D4CE` 制造键帽下缘、圆角 `--radius-xs`、mono 11px、文字 `--text-secondary`。

---

## 6. 组件视觉语言（浅色五态全重写）

### 6.1 按钮

| 类型 | 默认态 | 悬停态 | 按下态 | 禁用态 |
|---|---|---|---|---|
| **Primary** | 底 `--accent-gradient`（#2E5BFF→#2750E6 垂直），白字 600，高 36px，内边距 0 16px，圆角 `--radius-sm`，顶部 inset 高光 `rgba(255,255,255,0.18)`，`--shadow-accent` | 渐变上移为 `#3D67FF→#2E5BFF`（顶端白字仍 4.6:1）+ `--shadow-accent-lg` | 底压至 `#1F44D6→#1940BE`，阴影收回 `--shadow-accent`，transform `scale(0.98)` | 底 `#EFEFEB`，字 `--text-disabled`，无描边无阴影 |
| **Secondary** | 底 `#FFFFFF`，描边 `--border-strong` 1px，字 `--text-primary` | 底叠 `--surface-hover-veil`，描边升 `#B9B9B1` | 底 `--surface-sunken`，`scale(0.98)` | 同 Primary 禁用 |
| **Ghost** | 透明底无描边，字 `--text-secondary` | 底 `--surface-hover-veil`，字升 `--text-primary` | 底 `--surface-active-veil` | 字 `--text-disabled`，hover 无反应 |

尺寸系：高 32（small）/ 36（medium）/ 40（large）；图标按钮方形 36×36，图标 18px。

**ink 墨色面板上的按钮**（页脚「回到顶部」等落款行动点）：按 Phase 2R v1.1 定稿为 **Secondary 白底规格**——白键浮于墨底（白底 + `--border-strong` 1px + 字 `--text-primary`，hover 叠 `--surface-hover-veil`、描边升 `#B9B9B1`，按下底 `--surface-sunken`），与浅底 Secondary 完全同规格，不再使用 ghost-inverse 变体。若未来出现墨底上的幽灵操作，映射规则为：文字 `--text-inverse`、hover 底 `--ink-hover-veil`（`rgba(255,255,255,0.08)`）、active `--ink-active-veil`（`rgba(255,255,255,0.12)`）、禁用文字 `rgba(255,255,255,0.35)`——暗底 hover 反转为「加白」，与浅底 veil 同族、不引入新色相。

### 6.2 输入框

- 默认：底 `#FFFFFF`，描边 `--border-default` 1px，圆角 `--radius-sm`，高 36px，placeholder `--text-tertiary`，文字 `--text-primary`。
- 悬停：描边 `--border-strong`。
- 聚焦：描边 `--accent` + `0 0 0 3px rgba(46,91,255,0.15)`（比全局 ring 光晕略窄）。
- 错误：描边 `--danger` + `0 0 0 3px rgba(220,38,38,0.12)`，下方 12px/18px `--danger` 错误文案，距框 6px。
- 禁用：底 `#F7F7F4`，描边 `--border-subtle`，文字 `--text-disabled`。
- Dashboard 侧栏内搜索井：底 `--surface-sunken` 变体，其余同上。

### 6.3 卡片

- 容器：底 `#FFFFFF`，描边 `--border-default`，圆角 `--radius-lg`，内边距 24px，`--shadow-xs`。
- 悬停（可交互卡片）：描边 `--border-strong` + 阴影升至 `--shadow-sm`（150ms），**无位移**。
- 卡片标题区：H3 16px/600 + 右侧操作区，与正文间 `--border-subtle` 分隔。

### 6.4 下拉菜单（Select / Menu）

- 容器：白底 + `border-strong` + `--shadow-md`，圆角 `--radius-md`，内边距 6px，最小宽 200px，顶部 inset 白高光。
- 菜单项：高 32px，圆角 `--radius-xs`，文字 `--text-secondary`；hover 底 `--surface-hover-veil` 字升 primary；**选中项**底 `--accent-subtle`、文字 `--accent-deep`、左侧 16px 勾选图标（`--accent`）。
- 展开动画：scale `0.96→1` + 淡入 150ms，decelerate（沿用 §7）。

### 6.5 开关（Toggle）

- 轨道 44×24px，圆角 full；关闭态底 `#E3E3DE` 描边 `--border-strong`；开启态底 `--accent`（白滑块对钴蓝 5.2:1 ≥3:1 非文字达标）。
- 滑块 20px 纯白，开启后右移 20px（180ms standard）；按下时滑块横向拉伸至 24px，松开回弹。
- 禁用：关闭态底 `#F0F0EC` 滑块 `#D4D4CE`；开启态底 `rgba(46,91,255,0.4)`。

### 6.6 标签 / Badge

- 高 22px，内边距 0 8px，圆角 full，文字 12px/500，字距 +2%。
- 中性：底 `--surface-sunken`，文字 `--text-secondary`（6.5:1）。
- 语义：底对应 subtle 档，**文字用强调档**（如 success Badge：底 `#ECF8F0` + 字 `#166534`，6.5:1）。
- Accent 选中标签：底 `--accent-subtle`，文字 `--accent-deep`，描边 `rgba(46,91,255,0.30)` 1px。

### 6.7 表格行

- 行高 44px，行间分隔 `--border-subtle` 1px（hover 时升 `--border-default`）。
- 表头：高 40px，背景 `--surface-sunken`，文字 `--text-secondary` 12px/450，字距 +2%。
- 数据单元格 15px；数值列 mono `tabular-nums` 右对齐。
- 行 hover：整行叠 `--surface-hover-veil`（加灰）。
- 行选中：底 `--accent-subtle`，文字 `--text-primary`（15.8:1），最左 2px `--accent` 竖条。

### 6.8 顶部导航（展示页）

- 高 64px，吸顶，底 `rgba(247,247,244,0.82)` + `backdrop-blur(12px)`，底部 `--border-default` 1px。
- 左：Logo 标记 24px（钴蓝几何棱镜符号）+ 「PRISM」16px/650 拉丁 -1%；右：导航链接 14px `--text-secondary`，hover 升 primary，当前项 primary + 底部 2px `--accent` 指示条。
- 右端 GitHub 图标按钮 + Primary 小按钮（高 32px）。

### 6.9 侧边栏（Dashboard）

- 宽 240px，底 `#FFFFFF`，右侧 `--border-default` 1px（白侧栏浮在暖灰画布上，配 `--shadow-xs` 的 1px 右缘暗化即可，不加大影）。
- 分组标题：12px `--text-tertiary`，字距 +2.5%，上下 `--space-4`。
- 菜单项：高 36px，圆角 `--radius-sm`，图标 18px + 文字 14px，默认 `--text-secondary`；hover 底 `--surface-hover-veil`；**当前项**底 `--accent-subtle`、文字 `--accent-deep`、图标 `--accent`、左侧 2px `--accent` 竖条（长 20px 居中）。
- 底部用户区：`--border-subtle` 分隔线上方 12px，头像 28px 圆 + 名称 14px + 邮箱 12px tertiary。

---

## 7. 动效语言（骨架沿用 v1，无变化）

### 7.1 时长与缓动（全站仅两支曲线）

| Token | 值 | 用途 |
|---|---|---|
| `--duration-instant` | 80ms | 按下反馈、开关内部状态 |
| `--duration-fast` | 150ms | hover、focus ring、下拉展开 |
| `--duration-medium` | 240ms | 卡片展开、面板切换、toast |
| `--duration-slow` | 320ms | 页面/路由过渡、Modal 进出（上限） |
| `--ease-decelerate` | `cubic-bezier(0.16, 1, 0.3, 1)` | 一切「进入」 |
| `--ease-standard` | `cubic-bezier(0.4, 0, 0.2, 1)` | 一切「离开/过渡」 |

### 7.2 常见模式

- **Hover**：150ms / decelerate，只做颜色、描边、阴影变化，**不做位移**。
- **展开/收起**：scale `0.96→1` + 淡入 150ms 进入；收起 120ms 淡出 + `1→0.98`（standard）。
- **页面过渡**：320ms，新页 `opacity 0→1` + `translateY(8px→0)`，decelerate；旧页仅 150ms 淡出；Dashboard 内路由 240ms。
- **骨架屏**：底 `#FAFAF8` 块，shimmer `linear-gradient(90deg, transparent, rgba(23,24,28,0.04), transparent)` 平移循环 **1600ms**（浅色下用暖黑 4% 扫过）。
- **数字滚动**：600ms 从 0 计数至目标，mono + tabular-nums，easeOutExpo。
- **减少动效**：全站遵循 `prefers-reduced-motion: reduce`，届时仅保留 opacity 80ms。

---

## 8. 情绪版：Dashboard 概念页画面描述

> 供展示页 Hero 区直接引用的第一眼画面。

**整体构图**——1600px 宽的明亮驾驶舱：左侧 240px 白色侧边栏贴着一道 1px 暖灰竖线，像一页打印纸的装订边；中央画布是 `#F7F7F4` 的暖灰纸面，顶部 56px 顶栏里一枚白底键帽「⌘K」安静待命；内容区四周留着慷慨的 32px 呼吸位。

**第一眼焦点**——页面顶部四张白卡（12px 圆角、1px 暖灰描边、4% 透明度的暖黑影几乎不可察觉地托起纸面），每张卡内一个巨大的 mono 数字整齐列队，其中一张卡的数字与迷你趋势线被钴蓝点亮——这是整个画面**唯一的彩色焦点**，像白纸上的一条蓝墨线。

**中景**——左三分之二是主图表：网格线是 `#EDEDE9` 的极淡暖灰，数据线是钴蓝 1.5px，线下方 8% 透明度的蓝晕渐隐；坐标轴数字 mono 11px 暖灰。右三分之一是一列紧凑列表卡与一个环形进度，进度环钴蓝、轨道 `#E3E3DE`，旁边一枚 `#ECF8F0` 底、深绿字的「运行中」Badge。

**底部**——12 行数据表：表头沉在 `#F0F0EC` 灰带里、字距微张的 12px 灰字；数据行白底 44px 高、mono 右对齐数字，一行被 4% 暖黑 hover 纱轻轻压灰，另一行铺着淡蓝选中底、左缘一线 2px 钴蓝。

**光影氛围**——整个页面像一份高级印刷品：白卡浮在暖灰纸面上，阴影轻到只有 5%，纵深主要靠「白 / 暖灰 / 灰」三阶色差与 1px 描边；页脚是一条 `#17181C` 墨色横带，白字 logo 与链接像铅印般压在黑底上。95% 的画面是白与暖灰的秩序，彩色只有一条钴蓝、几枚语义淡彩——**白光进入棱镜，只折射出一条精确的蓝。**

---

## 9. 与 v1（OBSIDIAN）骨架差异表

> 骨架（字体栈 / 字阶 / 间距 / 圆角 / 断点 / 框架尺寸 / 动效）**全部沿用 v1，无变化**，Phase 2 体验规范继续有效。以下为仅有的差异项：

| 差异项 | v1 OBSIDIAN | v2 PRISM | 原因 |
|---|---|---|---|
| 阴影 token | 无体系（阴影仅辅助） | 新增 6 档（xs/sm/md/lg/accent/accent-lg） | 浅色下阴影是主要纵深手段 |
| hover 方向 | 加白提亮 `rgba(255,255,255,0.045)` | **加灰压暗** `rgba(23,24,28,0.04/0.07)` | 白底上提亮不可见 |
| focus 光晕 | `rgba(83,214,229,0.25)` | `rgba(46,91,255,0.18)`（alpha 降低） | 浅色下高 alpha 光晕发脏 |
| 新增 token | — | `--accent-deep`（tint 上文字）、`--surface-ink`（墨色面板）、`--border-data`（3:1 轴线）、`--surface-active-veil` | 浅色 tint 可读性 + 记忆点 |
| 浮层顶部高光 | `rgba(255,255,255,0.06)` 冷光 | `rgba(255,255,255,0.9)` 受光边 | 深浅底色语境互换 |
| 毛玻璃范围 | 仅吸顶 Header | 吸顶 Header + Dashboard 顶栏（两处） | 浅色毛玻璃更稳，仍克制 |
| shimmer 色 | 白 4% 扫过 | 暖黑 4% 扫过 | 底色反转 |
| 字重上限 / 字号 / 曲线 | 650 / 10 级 / 两支曲线 | **完全一致** | 骨架继承 |

---

## 附录：CSS 变量速查（前端直接取用）

```css
:root {
  color-scheme: light;

  /* Surface */
  --surface-sunken: #F0F0EC;
  --surface-base: #F7F7F4;
  --surface-raised: #FFFFFF;
  --surface-overlay: #FFFFFF;
  --surface-hover-veil: rgba(23,24,28,0.04);
  --surface-active-veil: rgba(23,24,28,0.07);
  --surface-ink: #17181C;
  --ink-hover-veil: rgba(255,255,255,0.08);
  --ink-active-veil: rgba(255,255,255,0.12);
  --focus-glow-ink: rgba(46,91,255,0.32);

  /* Text */
  --text-primary: #17181C;
  --text-secondary: #52535B;
  --text-tertiary: #6A6B73;
  --text-disabled: #9C9DA5;
  --text-inverse: #FFFFFF;

  /* Border */
  --border-subtle: #EDEDE9;
  --border-default: #E3E3DE;
  --border-strong: #D4D4CE;
  --border-data: #8A8B93;
  --border-accent-glow: rgba(46,91,255,0.30);

  /* Accent (Cobalt) */
  --accent: #2E5BFF;
  --accent-hover: #3D67FF;
  --accent-pressed: #1F44D6;
  --accent-deep: #2447CC;
  --accent-subtle: #EBF0FF;
  --accent-wash: rgba(46,91,255,0.08);
  --accent-gradient: linear-gradient(180deg, #2E5BFF 0%, #2750E6 100%);
  --accent-contrast-text: #FFFFFF;

  /* Semantic */
  --success: #15803D;  --success-strong: #166534;  --success-subtle: #ECF8F0;
  --warning: #B45309;  --warning-strong: #92400E;  --warning-subtle: #FCF3E3;
  --danger: #DC2626;   --danger-strong: #B91C1C;   --danger-subtle: #FDEDED;
  --info: #0F766E;     --info-strong: #115E59;     --info-subtle: #E6F4F2;

  /* Shadow */
  --shadow-xs: 0 1px 2px rgba(23,24,28,0.04);
  --shadow-sm: 0 1px 2px rgba(23,24,28,0.05), 0 2px 6px rgba(23,24,28,0.05);
  --shadow-md: 0 2px 6px rgba(23,24,28,0.06), 0 12px 24px rgba(23,24,28,0.09);
  --shadow-lg: 0 4px 12px rgba(23,24,28,0.08), 0 24px 48px rgba(23,24,28,0.14);
  --shadow-accent: 0 1px 2px rgba(30,64,175,0.24), 0 4px 12px rgba(46,91,255,0.22);
  --shadow-accent-lg: 0 2px 4px rgba(30,64,175,0.28), 0 8px 20px rgba(46,91,255,0.28);

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

— PRISM v2.0 · Phase 1R 完 · 替换 OBSIDIAN v1.0 —
