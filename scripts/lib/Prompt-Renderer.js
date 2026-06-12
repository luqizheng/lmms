// Prompt-Renderer.js
// 渲染 spec-to-code 提示词
const fs = require('node:fs');
const path = require('node:path');
const { ROOT, log } = require('./utils.js');

function getSpecs() {
  const root = path.join(ROOT, 'tests', 'specs');
  if (!fs.existsSync(root)) return [];
  const out = [];
  walk(root, out);
  return out;
}

function walk(dir, out) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (entry.isFile() && entry.name.endsWith('.yaml')) out.push(full);
  }
}

function renderSpecToCodePrompt() {
  const tmplPath = path.join(ROOT, 'scripts', 'prompts', 'spec-to-code.template.md');
  const tmpl = fs.readFileSync(tmplPath, 'utf8');
  const specs = getSpecs();
  let specsText = '';
  if (specs.length === 0) {
    specsText = '(无 YAML 规格)';
  } else {
    for (const f of specs) {
      specsText += `\n---\nFile: ${f}\n\n`;
      specsText += fs.readFileSync(f, 'utf8');
    }
  }
  const rendered = tmpl.replace(/\{\{SPECS\}\}/g, specsText);
  const outPath = path.join(ROOT, 'reports', 'spec-to-code-prompt.md');
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, rendered, 'utf8');
  log.green(`[prompt] rendered to ${outPath}`);
}

module.exports = { renderSpecToCodePrompt, getSpecs };
