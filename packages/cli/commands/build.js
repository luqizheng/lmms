'use strict';

const readline = require('node:readline/promises');
const { EXIT, UsageError } = require('../lib/errors.js');
const { DEFAULT_PLATFORM, parseBuildArgs, resolvePlan, buildImages, pruneBuildCache } = require('../lib/docker-build.js');

function planLines(plan) {
  return [
    `=== build plan ===`,
    `  version : ${plan.version || '(latest only)'}`,
    `  platform: ${plan.platforms.join(',')}`,
    `  push    : ${plan.mode === 'push' ? 'push' : plan.mode === 'load-and-push' ? 'load+push' : 'no'}`,
    `  registry: ${plan.registry}/${plan.repo}`,
    `  services: ${plan.services.join(', ')} (${plan.services.length})`,
  ];
}

async function promptInteractive(out) {
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  try {
    out.log('=== docker build 交互式（回车取默认） ===');
    const version = ((await rl.question('版本号 tag (如 1.0.0) [跳过] ')) || '').trim();
    const platform = ((await rl.question(`CPU 架构 (逗号分隔) [${DEFAULT_PLATFORM}] `)) || DEFAULT_PLATFORM).trim();
    const pushAns = ((await rl.question('推送? (y=推送 / n=不推 / l=load并推送) [y] ')) || 'y').toLowerCase();
    const svcAns = ((await rl.question('构建哪些服务 (逗号分隔) [全部] ')) || '').trim();

    const argv = [];
    if (version && version !== '跳过') argv.push('-v', version);
    argv.push('-p', platform);
    if (pushAns === 'l' || pushAns === 'load' || pushAns === 'lp') argv.push('--load-and-push');
    else if (pushAns.startsWith('y')) argv.push('--push');
    if (svcAns && svcAns !== '全部') argv.push(...svcAns.split(',').map((s) => s.trim()));
    return argv;
  } finally {
    rl.close();
  }
}

const commands = {
  'build images': {
    summary: 'build (and push) service images via buildx; `prune` clears cargo build cache',
    run: async (args, ctx) => {
      if (!ctx.root) throw new UsageError('build images requires a repo context (--cwd)');
      const parsed = parseBuildArgs(args);

      if (parsed.help) {
        return {
          data: { services: require('../lib/docker-build.js').SERVICES },
          human: [
            'usage: coder build images [services...] [-v <ver>] [--push|--load-and-push] [-p linux/amd64,...]',
            '       coder build images prune',
            `services: ${require('../lib/docker-build.js').SERVICES.join(', ')}`,
          ],
        };
      }

      if (parsed.prune) {
        const result = pruneBuildCache({ json: ctx.json, out: ctx.out });
        return {
          data: result,
          human: [`pruned: ${result.pruned.join(', ') || '(none)'}`, ...(result.failed.length ? [`failed: ${result.failed.join(', ')}`] : [])],
          exitCode: result.failed.length ? EXIT.FAIL : EXIT.OK,
        };
      }

      let plan = resolvePlan({ args: parsed, isTTY: process.stdin.isTTY === true });
      if (plan.interactive) {
        plan = resolvePlan({ args: parseBuildArgs(await promptInteractive(ctx.out)), isTTY: true });
      }
      for (const w of plan.warnings) ctx.out.warn(w);

      const result = buildImages({ root: ctx.root, plan, json: ctx.json, out: ctx.out });
      const human = [
        ...planLines(plan),
        `[done] built ${result.built.length}, failed ${result.failed.length}`,
        ...result.failed.map((f) => `  ✗ ${f.service}: ${f.reason}`),
      ];
      return {
        data: { plan, ...result },
        human,
        exitCode: result.failed.length ? EXIT.FAIL : EXIT.OK,
      };
    },
  },
};

module.exports = { commands };
