'use strict';

/**
 * lib/cli.js — argv parsing and dispatch for cms-release.
 * Exit codes follow the @digua/dist-tools convention: 0 / 1 (error) / 64 (usage).
 */

const repo = require('./repo');
const { COMMANDS } = require('./commands');

const EXIT_OK = 0;
const EXIT_ERROR = 1;
const EXIT_USAGE = 64;

const HELP = `cms-release — coder-micro-services 发布 CLI

用法:
  cms-release <command> [options]

命令:
  status                          查看 git tag / Makefile / CHANGELOG 版本对账
  publish [services...]           构建（可推送）服务镜像，委托 scripts/build-docker.js
  package                         打离线分发包，委托 release/dist-template/scripts/release.js
  bump <ver | --patch|--minor|--major>
                                  版本收口：改写 dist-template/Makefile，可选 --tag 打 git tag
  upgrade <dist-dir> <ver>        在解压后的分发包目录就地升级（bin/upgrade.js）

选项:
  -v <1.2.3|v1.2.3>   目标版本（缺省 = 最新 git tag 去 v）
  --push              构建后直接 buildx --push
  --load-and-push     本地留镜像 + docker push（仅单架构）
  --platform <spec>   目标平台（默认由 build-docker.js 决定）
  --no-images         package 时跳过 docker pull/save
  --tag               bump 时同时打 annotated git tag（本地）
  --force             bump 允许版本回退 / 脏工作区
  --dry-run           只打印将执行的命令 / 将写入的变更
  --root <path>       显式指定仓库根目录（默认从 cwd 向上查找）
  -h, --help          显示本帮助

示例:
  cms-release status
  cms-release publish -v 1.5.2 --load-and-push api-gateway token-service
  cms-release publish -v 1.5.2 --push              # 全部服务
  cms-release package -v 1.5.2
  cms-release bump --minor --tag
  cms-release upgrade /srv/coder-micro-services-1.5.2 1.5.3

版本规范: git tag 带 v（v1.5.2），镜像 tag / 包名不带 v（1.5.2）；入口两种都收。`;

const VALUE_FLAGS = new Set(['-v', '--platform', '--root']);
const BOOLEAN_FLAGS = new Set([
  '--push',
  '--load-and-push',
  '--no-images',
  '--tag',
  '--force',
  '--dry-run',
  '--patch',
  '--minor',
  '--major',
]);

function usageError(msg) {
  const err = new Error(msg);
  err.exitCode = EXIT_USAGE;
  return err;
}

// Returns { command, positionals, version, ...flags }.
function parseArgs(argv) {
  const parsed = {
    command: null,
    positionals: [],
    version: null,
    platform: null,
    root: null,
    help: false,
    push: false,
    loadAndPush: false,
    noImages: false,
    tag: false,
    force: false,
    dryRun: false,
    patch: false,
    minor: false,
    major: false,
  };

  const rest = argv.slice();
  while (rest.length > 0) {
    const arg = rest.shift();
    if (arg === '-h' || arg === '--help') {
      parsed.help = true;
    } else if (VALUE_FLAGS.has(arg)) {
      const val = rest.shift();
      if (val === undefined) throw usageError(`${arg} 缺少值`);
      if (arg === '-v') parsed.version = val;
      else if (arg === '--platform') parsed.platform = val;
      else parsed.root = val;
    } else if (BOOLEAN_FLAGS.has(arg)) {
      parsed[
        {
          '--push': 'push',
          '--load-and-push': 'loadAndPush',
          '--no-images': 'noImages',
          '--tag': 'tag',
          '--force': 'force',
          '--dry-run': 'dryRun',
          '--patch': 'patch',
          '--minor': 'minor',
          '--major': 'major',
        }[arg]
      ] = true;
    } else if (arg.startsWith('-') && arg !== '-') {
      throw usageError(`未知参数: ${arg}`);
    } else if (parsed.command === null) {
      parsed.command = arg;
    } else {
      parsed.positionals.push(arg);
    }
  }

  return parsed;
}

function main(argv) {
  let parsed;
  try {
    parsed = parseArgs(argv);
  } catch (err) {
    console.error(`${err.message}\n\n${HELP}`);
    return err.exitCode || EXIT_USAGE;
  }

  if (parsed.help || (!parsed.command && parsed.positionals.length === 0)) {
    console.log(HELP);
    return EXIT_OK;
  }
  if (!parsed.command) {
    console.error('缺少命令\n');
    console.log(HELP);
    return EXIT_USAGE;
  }
  const handler = COMMANDS[parsed.command];
  if (!handler) {
    console.error(`未知命令: ${parsed.command}\n`);
    console.log(HELP);
    return EXIT_USAGE;
  }

  // upgrade operates on an external dist dir and needs no repo root.
  if (parsed.command !== 'upgrade') {
    const root = parsed.root ? require('node:path').resolve(parsed.root) : repo.findRoot(process.cwd());
    if (!root) {
      console.error('未定位到仓库根目录（缺少 scripts/build-docker.js）— 用 --root 显式指定，或到框架仓内执行');
      return EXIT_ERROR;
    }
    parsed.root = root;
  }

  try {
    return handler({ args: parsed, root: parsed.root }) || EXIT_OK;
  } catch (err) {
    console.error(err && err.stack ? err.stack : err);
    return EXIT_ERROR;
  }
}

module.exports = { parseArgs, main, HELP, EXIT_OK, EXIT_ERROR, EXIT_USAGE };
