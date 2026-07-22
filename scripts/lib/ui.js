// scripts/lib/ui.js
// 统一 UI helpers：chalk 颜色 + ora spinner + 语义化日志
//
// 用法：
//   const { chalk, success, warn, error, info, section, banner, spinner } = require('./lib/ui');
//   success('done');
//   const sp = spinner('downloading...').start();
//   sp.succeed('downloaded');
//
// spinner 在非 TTY / CI 环境自动降级为普通日志，防止 spinner 帧撞到子进程输出。

const chalk = require('chalk');
const ora = require('ora');

// 段落标题：▸ + 加粗青色
function section(title) {
  console.log('\n' + chalk.bold.cyan(`▸ ${title}`));
}

// 顶部横幅：白字蓝底
function banner(title) {
  console.log('\n' + chalk.bold.bgBlue.white(` ${title} `) + '\n');
}

// 语义化日志：4 种带图标
function success(msg) { console.log(chalk.green('✓'), msg); }
function warn(msg)    { console.log(chalk.yellow('⚠'), msg); }
function error(msg)   { console.error(chalk.red('✗'), msg); }
function info(msg)    { console.log(chalk.cyan('ℹ'), msg); }

// spinner 工厂：非 TTY / CI 时返回降级版
function spinner(text) {
  const isCiOrNonTty = !process.stdout.isTTY || process.env.CI;
  if (isCiOrNonTty) {
    // 降级：只打印 start / succeed / fail / warn / info 的文本
    const dummy = {
      text,
      isSpinning: false,
      start(t) { if (t) dummy.text = t; console.log(chalk.gray('…'), dummy.text); return dummy; },
      succeed(msg) { console.log(chalk.green('✓'), msg || dummy.text); return dummy; },
      fail(msg)    { console.error(chalk.red('✗'), msg || dummy.text); return dummy; },
      warn(msg)    { console.log(chalk.yellow('⚠'), msg || dummy.text); return dummy; },
      info(msg)    { console.log(chalk.cyan('ℹ'), msg || dummy.text); return dummy; },
      stop() { return dummy; },
      clear() { return dummy; },
      render() { return dummy; },
    };
    return dummy;
  }
  return ora({ text, spinner: 'dots' });
}

module.exports = {
  chalk,
  section,
  banner,
  success,
  warn,
  error,
  info,
  spinner,
};
