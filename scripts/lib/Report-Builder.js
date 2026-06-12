// Report-Builder.js
// 解析 cargo/go 的 JSON 输出，生成 summary.md 与 fix-prompt.md
const fs = require('node:fs');
const path = require('node:path');
const { ROOT, log, formatTimestamp } = require('./utils.js');

function readJsonLines(p) {
  if (!fs.existsSync(p)) return [];
  const text = fs.readFileSync(p, 'utf8');
  const out = [];
  for (const line of text.split(/\r?\n/)) {
    const t = line.trim();
    if (!t) continue;
    try { out.push(JSON.parse(t)); } catch { /* skip */ }
  }
  return out;
}

function parseCargoEvents(jsonPath, logPath) {
  const cases = [];
  const textPath = (logPath && fs.existsSync(logPath)) ? logPath : jsonPath;
  if (!fs.existsSync(textPath)) return cases;
  const text = fs.readFileSync(textPath, 'utf8');
  for (const line of text.split(/\r?\n/)) {
    const m = line.match(/^\s*test\s+([^\s\.]+?)(?:\s*\.\.\.\s*)?\s+(ok|FAILED|ignored|ok\.)\s*$/);
    if (!m) continue;
    const name = m[1];
    const st = m[2].replace(/\.$/, '');
    const status = st === 'ok' ? 'pass' : st === 'FAILED' ? 'fail' : st === 'ignored' ? 'ignored' : 'pass';
    if (!cases.find((c) => c.name === name && c.status === status)) {
      cases.push({ name, status, message: null, exec_time_ms: 0 });
    }
  }
  return cases;
}

function parseGoEvents(jsonPath) {
  const cases = [];
  for (const ev of readJsonLines(jsonPath)) {
    if (!['pass', 'fail', 'skip'].includes(ev.Action)) continue;
    const status = ev.Action === 'pass' ? 'pass' : ev.Action === 'fail' ? 'fail' : 'ignored';
    cases.push({
      name: ev.Test,
      status,
      message: ev.Output,
      exec_time_ms: ev.Elapsed ? Math.round(ev.Elapsed * 1000) : 0,
    });
  }
  return cases;
}

function getParserForService(baseName) {
  const svc = baseName.replace(/\.(unit|integration)$/, '');
  if (['api-gateway', 'token-server'].includes(svc)) return 'cargo';
  return 'go';
}

function buildSummary(runDir) {
  const lines = [`# Test Run ${formatTimestampForDisplay(new Date())}`, '', '## 总览', ''];
  lines.push('| 服务 | 类型 | 通过 | 失败 | 跳过 |');
  lines.push('|------|------|------|------|------|');
  const allFailing = [];
  const jsonFiles = fs.readdirSync(runDir).filter((n) => n.endsWith('.json')).map((n) => path.join(runDir, n));
  for (const f of jsonFiles) {
    const base = path.basename(f, '.json');
    const parser = getParserForService(base);
    const logPath = path.join(runDir, `${base}.log`);
    const cases = parser === 'cargo' ? parseCargoEvents(f, logPath) : parseGoEvents(f);
    const pass = cases.filter((c) => c.status === 'pass').length;
    const fail = cases.filter((c) => c.status === 'fail').length;
    const ign  = cases.filter((c) => c.status === 'ignored').length;
    const type = base.includes('unit') ? 'unit' : 'integration';
    const svc  = base.replace(/\.(unit|integration)$/, '');
    lines.push(`| ${svc} | ${type} | ${pass} | ${fail} | ${ign} |`);
    for (const c of cases.filter((x) => x.status === 'fail')) {
      allFailing.push({ Service: svc, Type: type, Name: c.name, Message: c.message });
    }
  }
  lines.push('');
  if (allFailing.length > 0) {
    lines.push('## 失败详情');
    lines.push('');
    for (const fail of allFailing) {
      const msg = fail.Message ? fail.Message.split(/\r?\n/).slice(0, 20).join('\n') : '(无消息)';
      lines.push(`### ${fail.Service} [${fail.Type}] :: ${fail.Name}`);
      lines.push('``````');
      lines.push(msg);
      lines.push('``````');
      lines.push('');
    }
  }
  const summaryPath = path.join(runDir, 'summary.md');
  fs.writeFileSync(summaryPath, lines.join('\n'), 'utf8');
  return { SummaryPath: summaryPath, Failing: allFailing };
}

function buildFixPrompt(runDir, failing) {
  const tmpl = fs.readFileSync(path.join(ROOT, 'scripts', 'prompts', 'fix-tests.template.md'), 'utf8');
  const failingText = failing.map((f) => `- ${f.Service} [${f.Type}] :: ${f.Name}`).join('\n');
  const errorText = failing.map((f) => {
    const msg = f.Message ? f.Message.split(/\r?\n/).slice(0, 20).join('\n') : '(无消息)';
    return `### ${f.Service} [${f.Type}] :: ${f.Name}\n\`\`\`\`\`\`\n${msg}\n\`\`\`\`\`\``;
  }).join('\n\n');
  const files = Array.from(new Set(failing.map((f) => {
    if (f.Type === 'integration') {
      if (['api-gateway', 'token-server'].includes(f.Service)) {
        return `services/${f.Service}/tests/${f.Name}.rs`;
      }
      return `services/${f.Service}/${f.Name}_test.go`;
    }
    if (['api-gateway', 'token-server'].includes(f.Service)) {
      return `services/${f.Service}/src/${f.Name}.rs`;
    }
    return `services/${f.Service}/${f.Name}_test.go`;
  }))).join('\n');
  const rendered = tmpl
    .replace(/\{\{FAILING_TESTS\}\}/g, failingText)
    .replace(/\{\{ERROR_DETAILS\}\}/g, errorText)
    .replace(/\{\{RELATED_FILES\}\}/g, files);
  const fixPath = path.join(runDir, 'fix-prompt.md');
  fs.writeFileSync(fixPath, rendered, 'utf8');
  const latest = path.join(ROOT, 'reports', 'latest-fix-prompt.md');
  fs.copyFileSync(fixPath, latest);
  return fixPath;
}

function formatTimestampForDisplay(d) {
  const pad = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

module.exports = { buildSummary, buildFixPrompt, parseCargoEvents, parseGoEvents };
