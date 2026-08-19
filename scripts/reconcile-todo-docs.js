#!/usr/bin/env node
/**
 * reconcile-todo-docs.js — TODO.md ↔ docs-site/guides/* 状态行漂移检测
 *
 * 目标：守住 ai-context.md §11 的核心规则——"文档与代码冲突时：改文档，并在 commit message 注明"。
 * 项目里 TODO.md（实时债务清单）+ docs-site/guides/*.md（操作指南 / 顶部状态行）+ ai-context.md（状态快照）
 * 是三处事实源；任意两处不一致都说明有未同步的 commit。
 *
 * 设计原则：
 *   - 只检测、不自动改：drift 报告交给 AI/人在 commit 时处理；自动写文件会变成"AI 改 AI 自己"。
 *   - 两类规则：
 *       Rule 1（HARD）：TODO.md 父行描述"待 ..."但所有子项已 `[x]`。
 *                      例：`- 数据库 schema ... — G3 7/7 端点接入完成（待集成测试 + 文档同步）`
 *                          + 子项 `[x] 集成测试 + 文档同步 — ... 已完整`
 *                          → 漂移：父行末尾的"待 XXX"已不成立。
 *       Rule 2（HARD）：docs-site/guides/*.md 顶部 `> **状态**：<token>` 已标记落地（v\d+ / 可执行 / 落地 /
 *                      closed / complete / production），但 TODO.md 中对应 topic 仍 `[ ]`。
 *                      用内置映射 KNOWN_TOPICS 串联 TODO.md 的关键字与 guide 路径。
 *       Rule 3（advisory）：TODO.md `[ ]` 项在 KNOWN_TOPICS 中找不到任何 guide 关联——可能是真新工作
 *                      （如 Phase 7 K3s），也可能缺文档；只 warn，不 fail。
 *
 *   - 退出码：0 = 全 OK / 仅 advisory，1 = 至少一条 HARD drift。
 *
 * 用法：
 *   node scripts/reconcile-todo-docs.js           # 检测 + 报告
 *   node scripts/reconcile-todo-docs.js --verbose # 含每条 [x] 项的核对日志
 *
 * 约束：
 *   - Node 18+，无外部依赖（chalk / ora 也不引入，保持与 check-doc-links.js 一致的零依赖风格）。
 *   - 仅 read，不 write；不输出到 stdout 之外的文件。
 *   - 不解析 ai-context.md（§11 强调"以代码/实现为准"，ai-context 是给人读的快照，不是机器源）。
 */

'use strict';

const fs = require('fs');
const path = require('path');

const REPO_ROOT = path.resolve(__dirname, '..');
const TODO_PATH = path.join(REPO_ROOT, 'TODO.md');
const GUIDES_DIR = path.join(REPO_ROOT, 'docs-site', 'guides');
const DECISIONS_DIR = path.join(REPO_ROOT, 'docs-site', 'decisions');
const RUNBOOKS_DIR = path.join(REPO_ROOT, 'docs-site', 'runbooks');

const VERBOSE = process.argv.includes('--verbose');

// ANSI color（手写，不引 chalk）：CI / TTY 都向下兼容
const C = {
  red:    (s) => `\x1b[31m${s}\x1b[0m`,
  green:  (s) => `\x1b[32m${s}\x1b[0m`,
  yellow: (s) => `\x1b[33m${s}\x1b[0m`,
  cyan:   (s) => `\x1b[36m${s}\x1b[0m`,
  dim:    (s) => `\x1b[2m${s}\x1b[0m`,
  bold:   (s) => `\x1b[1m${s}\x1b[0m`,
};

function info(msg)   { console.log(C.cyan('ℹ'), msg); }
function ok(msg)     { console.log(C.green('✔'), msg); }
function warn(msg)   { console.log(C.yellow('⚠'), msg); }
function hard(msg)   { console.log(C.red('✖ HARD drift'), msg); }
function dim(msg)    { console.log(C.dim(msg)); }

// —— 状态行分类 ——
const STATUS_LANDED = [
  /\bv\d+\b/i,
  /可执行/,
  /落地/,
  /closed/i,
  /complete/i,
  /production/i,
  /approved/i,
  /已采纳/,
  /已.*合并/,
];
const STATUS_PENDING = [
  /草案/,
  /待.*评审/,
  /待.*确认/,
  /待.*补充/,
  /待.*运维/,
  /待.*负责人/,
  /待.*规范/,
  /待推进/,
  /\[ ?待 ?推进 ?\]/,
  /pending/i,
  /draft/i,
  /初版\s*·\s*可执行/, // 既是初版又可执行，落地
];

function classifyStatus(text) {
  // 命中"已落地"关键词（按出现顺序优先级）
  for (const re of STATUS_LANDED) {
    if (re.test(text)) return 'landed';
  }
  for (const re of STATUS_PENDING) {
    if (re.test(text)) return 'pending';
  }
  return 'unknown';
}

// —— TODO.md 解析 ——
function parseTodo(content) {
  const lines = content.split('\n');
  const items = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    // 匹配 "- [x]" 或 "- [ ]"（允许前导空格）
    const m = line.match(/^(\s*)- \[(x| )\]\s+(.*)$/i);
    if (!m) continue;
    const indent = m[1].length;
    const done = m[2].toLowerCase() === 'x';
    const text = m[3].trim();
    items.push({ line: i + 1, indent, done, text, raw: line });
  }
  return items;
}

// 父行（无 checkbox 的 `- ...` 或 `#### ...`）携带"待 XXX" / "未完成" 等标记
function findPendingMarkersInHeaders(content) {
  const lines = content.split('\n');
  const out = [];
  // 父行模式：以 `- ` / `* ` / 数字开头，但**不带** checkbox
  const headerRe = /^(\s*)([-*]|\d+\.)\s+(?!\[[ x]\])(.*)$/;
  // 待办关键词（在父行末尾括号或行内出现）
  const pendingKeywords = [
    /（待[^）]+）/,
    /\(待[^)]+\)/,
    /（未完成）/,
    /（未跑[^）]*）/,
    /待.*同步/,
    /（TODO）/,
    /\[ ?待推进 ?\]/,
  ];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (/^\s*-\s+\[[ x]\]/.test(line)) continue; // skip checkbox lines
    const m = line.match(headerRe);
    if (!m) continue;
    const text = m[3];
    for (const re of pendingKeywords) {
      if (re.test(text)) {
        out.push({ line: i + 1, indent: m[1].length, text: text.trim(), raw: line, marker: re.source });
        break;
      }
    }
  }
  return out;
}

// 给定父行（无 checkbox），找出紧跟在它后面、缩进更深的 checkbox 子项集合
function childrenOf(Parent, items) {
  return items.filter((it) => it.indent > Parent.indent);
}

// —— guide / decision 状态行解析 ——
function readStatusLine(absPath) {
  try {
    const content = fs.readFileSync(absPath, 'utf8');
    const lines = content.split('\n').slice(0, 20);
    for (const line of lines) {
      // `> **状态**：...` 或 `> **Status**: ...`
      const m = line.match(/^>\s+\*\*(状态|Status)\*\*\s*[:：]\s*(.+?)\s*$/i);
      if (m) return { status: m[2].trim(), classification: classifyStatus(m[2]) };
      // 也匹配简单 `> 状态：...` 或 `> 状态: ...`（兼容老格式 ADR / decision 顶部）
      const m2 = line.match(/^>\s*(状态|Status)\s*[:：]\s*(.+?)\s*$/i);
      if (m2) return { status: m2[2].trim(), classification: classifyStatus(m2[2]) };
    }
  } catch { /* file not found, etc. */ }
  return null;
}

// —— 已知映射：TODO.md topic ↔ guide/decision/runbook 路径 ——
const KNOWN_TOPICS = [
  {
    key: 'migrations-state',
    todoPattern: /migrations[\\/]+state/,
    title: '数据库 schema 当前版本/历史查询端点（migrations/state 端点）',
    docs: [
      { path: path.join(GUIDES_DIR, 'migrations-state.md'), required: 'landed' },
    ],
  },
  {
    key: 'prue-backend-sqlite',
    todoPattern: /prue_backend.*SQLite|prue.*backend.*sqlite/i,
    title: 'prue_backend SQLite 分支决策',
    docs: [
      { path: path.join(DECISIONS_DIR, '2026-07-08-prue-backend-sqlite-decision-design.md'), required: 'landed' },
    ],
  },
  {
    key: 'envoy-pingora-eval',
    todoPattern: /自研 router.*Envoy.*Pingora|Envoy.*Pingora.*评估/,
    title: '评估自研 router 替换为 Envoy / Pingora 的成本/收益',
    docs: [
      { path: path.join(DECISIONS_DIR, 'ADR-006-gateway-long-term-strategy.md'), required: 'landed' },
    ],
  },
  {
    key: 'gateway-redis-ratelimit',
    todoPattern: /gateway.*限流.*Redis|限流统一走 Redis/,
    title: 'gateway 限流统一走 Redis',
    docs: [
      // 无独立 guide；以 ADR-006 或 USER_MANUAL.md 落地即可。advisory only。
    ],
  },
  {
    key: 'secret-rotation',
    todoPattern: /密钥轮换|secret.rotation/i,
    title: '密钥轮换机制 SOP',
    docs: [
      { path: path.join(RUNBOOKS_DIR, 'secret-rotation.md'), required: 'landed' },
    ],
  },
  {
    key: 'backup-runbook',
    todoPattern: /备份.*Runbook|backup.*runbook/i,
    title: '备份 Runbook',
    docs: [
      { path: path.join(RUNBOOKS_DIR, 'backup.md'), required: 'landed' },
    ],
  },
  {
    key: 'mysql-replication',
    todoPattern: /MySQL.*主从|mysql.*replication/i,
    title: 'MySQL 主从',
    docs: [
      { path: path.join(RUNBOOKS_DIR, 'mysql-replication.md'), required: 'landed' },
    ],
  },
  {
    key: 'log-aggregation',
    todoPattern: /集中式日志收集|ELK|Loki|阿里云 SLS/,
    title: '集中式日志收集',
    docs: [
      { path: path.join(RUNBOOKS_DIR, 'log-aggregation.md'), required: 'landed' },
    ],
  },
];

// —— Rule 1: TODO.md 父行"待 XXX" + 子项全 `[x]` → HARD drift ——
function rule1Check(headers, items) {
  const drifts = [];
  const itemsByLine = new Map(items.map((it) => [it.line, it]));
  for (const h of headers) {
    // 找 h.line 之后第一个 indent <= h.indent 的 checkbox 行，确定父项范围 end
    let endLine = Infinity;
    for (const it of items) {
      if (it.line > h.line && it.indent <= h.indent) {
        endLine = it.line - 1;
        break;
      }
    }
    // 父项下 indent > h.indent 的 items = 该父项下的子项（包含嵌套层）
    const kids = items.filter((it) => it.line > h.line && it.line <= endLine && it.indent > h.indent);
    if (kids.length === 0) continue;
    const total = kids.length;
    const done = kids.filter((it) => it.done).length;
    if (done === total) {
      drifts.push({
        rule: 'R1',
        todoLine: h.line,
        todoText: h.text,
        reason: `父行末尾含"待 ..."标记但全部 ${total} 个子项已 [x]`,
        evidence: kids.slice(-2).map((k) => `[x] ${k.text.slice(0, 60)}… (line ${k.line})`).join('  |  '),
      });
    } else if (VERBOSE) {
      dim(`R1 advisory: ${h.text.slice(0, 60)}… → 子项 ${done}/${total} [x]（待办真实存在，跳过）`);
    }
  }
  return drifts;
}

// —— Rule 2: TODO.md `[ ]` 项 ↔ guide 状态行 ——
function rule2Check(items) {
  const drifts = [];
  for (const topic of KNOWN_TOPICS) {
    // 找到匹配 topic 的 TODO 项
    const todoMatches = items.filter((it) => !it.done && topic.todoPattern.test(it.text));
    if (todoMatches.length === 0) continue;

    // 检查每个关联文档的状态
    for (const doc of topic.docs) {
      const status = readStatusLine(doc.path);
      if (!status) {
        if (VERBOSE) dim(`R2 advisory: ${topic.key} → ${path.relative(REPO_ROOT, doc.path)} 无状态行`);
        continue;
      }
      if (status.classification === 'landed') {
        drifts.push({
          rule: 'R2',
          todoLine: todoMatches[0].line,
          todoText: todoMatches[0].text,
          reason: `关联文档 ${path.relative(REPO_ROOT, doc.path)} 顶部状态行标记"已落地"（${status.status}），但 TODO.md 仍 [ ]`,
          evidence: `status: "${status.status}"`,
        });
      } else if (status.classification === 'pending' && VERBOSE) {
        dim(`R2 advisory: ${topic.key} → doc 标记 ${status.status}，与 TODO [ ] 一致`);
      }
    }
  }
  return drifts;
}

// —— Rule 3: TODO.md `[ ]` 项 ↔ KNOWN_TOPICS 全无映射 → advisory ——
function rule3Advisory(items) {
  const advisories = [];
  for (const it of items) {
    if (it.done) continue;
    const text = it.text;
    // 跳过明显"已设计但推迟" 的条目（如 R2.1/R2.2 / Phase 7 K3s）
    if (/R2\.|延后至 Roadmap 2\.0|Phase 7|K3s|Kubernetes/i.test(text)) continue;
    // 跳过纯占位 / onboarding
    if (/^\(?待.*补充\)?|^\(?待.*评审\)?/.test(text)) continue;
    // 找有没有 topic 命中
    const matched = KNOWN_TOPICS.some((t) => t.todoPattern.test(text));
    if (!matched) {
      advisories.push({ line: it.line, text: text.slice(0, 80) });
    }
  }
  return advisories;
}

// —— main ——
function main() {
  console.log(C.bold('reconcile-todo-docs') + C.dim(' — TODO.md ↔ docs 状态行漂移检测'));
  console.log('');

  if (!fs.existsSync(TODO_PATH)) {
    console.error(`TODO.md not found at ${TODO_PATH}`);
    process.exit(1);
  }

  const todoContent = fs.readFileSync(TODO_PATH, 'utf8');
  const items = parseTodo(todoContent);
  const headers = findPendingMarkersInHeaders(todoContent);

  info(`TODO.md: ${items.length} checkbox items, ${headers.length} 父行含"待..."标记`);

  const r1 = rule1Check(headers, items);
  const r2 = rule2Check(items);
  const r3 = rule3Advisory(items);

  const hardDrifts = [...r1, ...r2];

  if (hardDrifts.length === 0) {
    ok(`无 HARD drift（${r1.length} R1 / ${r2.length} R2 clean）`);
  } else {
    console.log('');
    console.log(C.bold(C.red(`发现 ${hardDrifts.length} 条 HARD drift：\n`)));
    for (const d of hardDrifts) {
      hard(`[${d.rule}] TODO.md:${d.todoLine}`);
      console.log(`     ${C.dim(d.todoText)}`);
      console.log(`     ${C.yellow('原因：')}${d.reason}`);
      if (d.evidence) console.log(`     ${C.dim('证据：')}${d.evidence}`);
      console.log('');
    }
  }

  if (r3.length > 0) {
    console.log('');
    warn(`${r3.length} 条 [ ] 项未映射到任何已知 guide（advisory，真新工作或缺文档）：`);
    // 最多列 10 条避免刷屏
    for (const a of r3.slice(0, 10)) {
      console.log(`     ${C.dim(`L${a.line}`)} ${a.text}…`);
    }
    if (r3.length > 10) {
      console.log(`     ${C.dim(`… 另有 ${r3.length - 10} 条`)}`);
    }
  }

  console.log('');
  process.exit(hardDrifts.length > 0 ? 1 : 0);
}

main();