#!/usr/bin/env node
// 在主机上编译各 service 的 Rust 二进制（不进 Docker 编译容器）。
//
// 与 scripts/build-docker.js 的分工：
//   - build-docker.js 出的是发布镜像（Linux/musl，产物可直接进容器）。
//   - build-cargo.js 出的是主机本地产物，用于本机跑服务 / 快速验证能否编译通过。
//
// 用法：
//   node scripts/build-cargo.js                  # 全部 service，release
//   node scripts/build-cargo.js --debug          # 全部 service，debug
//   node scripts/build-cargo.js iam-service      # 指定 service（可多个）
//   node scripts/build-cargo.js --backend native # 强制本机 cargo（Windows 默认走 WSL）
//   node scripts/build-cargo.js --distro debian-rust
const fs = require('node:fs');
const path = require('node:path');
const { chalk, section } = require('./lib/ui.js');
const { resolveRunner, toWslPath } = require('./lib/host-runner.js');
const { SERVICE_DIRS, SERVICE_PKG } = require('./lib/services.js');

const ROOT = path.join(__dirname, '..');

// utoipa-swagger-ui build.rs 需要的 vendor zip（否则 build.rs 会去连 github.com）。
const SWAGGER_UI_ZIP = path.join(ROOT, 'infrastructure', 'vendor', 'swagger-ui-v5.17.14.zip');
const SWAGGER_UI_ENV_KEY = 'SWAGGER_UI_DOWNLOAD_URL';

function parseArgs(argv) {
  const opts = {
    profile: 'release',
    backend: null,
    distro: process.env.WSL_DISTRO || null,
    services: [],
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case '--release': opts.profile = 'release'; break;
      case '--debug': opts.profile = 'debug'; break;
      case '--backend': opts.backend = argv[++i] || null; break;
      case '--distro': opts.distro = argv[++i] || null; break;
      default:
        if (a.startsWith('-')) {
          console.warn(chalk.yellow(`[warn] 忽略未知参数: ${a}`));
        } else {
          opts.services.push(a);
        }
    }
  }
  return opts;
}

// 有 vendor zip 就注入 file:// URL，避免 build.rs 走网络。
function swaggerEnv(runner) {
  if (process.env[SWAGGER_UI_ENV_KEY]) {
    return { [SWAGGER_UI_ENV_KEY]: process.env[SWAGGER_UI_ENV_KEY] };
  }
  if (!fs.existsSync(SWAGGER_UI_ZIP)) {
    console.warn(chalk.yellow(`[swagger-vendor] zip 不存在: ${SWAGGER_UI_ZIP}`));
    console.warn(chalk.yellow('[swagger-vendor] 先跑 `pnpm run assets:download`'));
    return {};
  }
  const abs = runner.backend === 'wsl' ? toWslPath(SWAGGER_UI_ZIP) : SWAGGER_UI_ZIP;
  return { [SWAGGER_UI_ENV_KEY]: `file://${abs.startsWith('/') ? '' : '/'}${abs}` };
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const runner = resolveRunner({ backend: opts.backend, distro: opts.distro });

  const targets = opts.services.length > 0
    ? opts.services.filter((s) => SERVICE_DIRS[s])
    : Object.keys(SERVICE_DIRS);
  const unknown = opts.services.filter((s) => !SERVICE_DIRS[s]);
  for (const u of unknown) console.warn(chalk.yellow(`[warn] 未知 service: ${u}`));

  console.log(chalk.bold('\n=== cargo build (host) ==='));
  console.log(`  backend : ${runner.backend}${runner.distro ? ` (${runner.distro})` : ''}`);
  console.log(`  repo    : ${runner.repoPath}`);
  console.log(`  profile : ${opts.profile}`);
  console.log(`  services: ${targets.join(', ')} (${targets.length})`);

  const env = swaggerEnv(runner);
  const failed = [];
  for (const svc of targets) {
    section(`build ${svc}`);
    const args = ['build'];
    if (opts.profile === 'release') args.push('--release');
    args.push('-p', SERVICE_PKG[svc]);
    const res = runner.run('cargo', args, { env });
    if (res.status !== 0) failed.push(svc);
  }

  section('summary');
  console.log(`total : ${targets.length}`);
  console.log(`pass  : ${chalk.green(targets.length - failed.length)}`);
  console.log(`fail  : ${failed.length ? chalk.red(failed.length) : failed.length}`);
  for (const f of failed) console.log(`  ${chalk.red('-')} ${f}`);
  process.exit(failed.length ? 1 : 0);
}

main().catch((err) => {
  console.error(`build-cargo failed: ${err.message}`);
  if (err.stack) console.error(err.stack);
  process.exit(1);
});
