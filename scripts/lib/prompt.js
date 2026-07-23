// scripts/lib/prompt.js
// prompts 封装：语义化交互 + non-interactive 短路 + Ctrl+C 处理
//
// 用法：
//   const { input, confirm, select, multiselect, password } = require('./lib/prompt');
//   const port = await input('端口号', '3000', { nonInteractive: false });
//   const ok = await confirm('继续吗？', true);
//   const svc = await select('选服务', [{title:'iam',value:'iam'}, ...]);
//
// non-interactive: true 时短路，直接返回 initial（或 choices[initial].value）
// Ctrl+C: 打印黄色取消提示后以 exit code 130 退出

const prompts = require('prompts');
const chalk = require('chalk');

function onCancel() {
  console.error(chalk.yellow('\n✗ 操作被取消'));
  process.exit(130);
}

async function ask(config, opts = {}) {
  const { nonInteractive = false } = opts;
  if (nonInteractive) {
    // 短路：select/multiselect 用 choices[initial].value；其他直接用 initial
    let value = config.initial;
    if (config.type === 'select' && Array.isArray(config.choices)) {
      const idx = typeof config.initial === 'number' ? config.initial : 0;
      value = config.choices[idx] && config.choices[idx].value;
    } else if (config.type === 'multiselect' && Array.isArray(config.choices)) {
      value = config.choices.filter((c) => c.selected).map((c) => c.value);
    }
    return { [config.name]: value };
  }
  return prompts(config, { onCancel });
}

async function input(message, initial, opts = {}) {
  const r = await ask({ type: 'text', name: 'v', message, initial }, opts);
  return r.v;
}

async function confirm(message, initial = false, opts = {}) {
  const r = await ask({ type: 'confirm', name: 'v', message, initial }, opts);
  return r.v;
}

async function select(message, choices, initial = 0, opts = {}) {
  const r = await ask({ type: 'select', name: 'v', message, choices, initial }, opts);
  return r.v;
}

async function multiselect(message, choices, opts = {}) {
  const r = await ask(
    { type: 'multiselect', name: 'v', message, choices, hint: '空格选择, 回车确认' },
    opts,
  );
  return r.v;
}

async function password(message, opts = {}) {
  const r = await ask({ type: 'password', name: 'v', message }, opts);
  return r.v;
}

module.exports = {
  ask,
  input,
  confirm,
  select,
  multiselect,
  password,
};
