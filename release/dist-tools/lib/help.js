'use strict';

/**
 * lib/help.js — help text printed by `--help`.
 *
 * Keeps user-facing prose in one place so it's easy to edit / translate.
 */

function printHelp() {
  const lines = [
    'cms-sync — 把 coder-micro-services 上游发布包同步到当前项目',
    '',
    '用法：',
    '  cms-sync [--dry-run] <tarball-or-url>',
    '  cms-sync --check',
    '  cms-sync --help',
    '',
    '选项：',
    '  --dry-run          只显示会做什么，不修改文件',
    '  --check            显示当前同步状态',
    '  --root <path>      项目根目录 (默认: cwd)',
    '  --policy <path>    策略文件路径 (默认: <root>/.upstream-sync.yaml)',
    '  --lock <path>      lock 文件路径 (默认: <root>/.upstream.lock)',
    '  -h, --help         显示本帮助',
    '',
    '退出码：',
    '  0  同步成功（无冲突）',
    '  1  同步失败（IO 错误等）',
    '  2  存在合并冲突，需要手动解决',
    '  64 参数错误',
    '',
    '配合 .upstream-sync.yaml 和 .upstream.lock 工作。详见 README.md。',
  ];
  console.log(lines.join('\n'));
}

module.exports = { printHelp };