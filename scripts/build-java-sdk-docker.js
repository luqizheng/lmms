// 在 docker 容器中编译 / 测试 / 打包 sdk/java/（volume mapping 模型）。
// 源码与 ~/.m2 缓存全部用挂载，不构建项目镜像，行为对齐 scripts/run-tests-docker.js。
//
// 用法：
//   node scripts/build-java-sdk-docker.js                 # 默认 verify（编译 + 测试 + 打包）
//   node scripts/build-java-sdk-docker.js test            # 只跑测试
//   node scripts/build-java-sdk-docker.js compile         # 只编译
//   node scripts/build-java-sdk-docker.js package         # 编译 + 打包（跳过测试）
//   node scripts/build-java-sdk-docker.js deploy          # 部署到内网 Maven 仓库
//   node scripts/build-java-sdk-docker.js test -- -pl user-service-user-sdk    # 传给 mvn 的额外参数
//   node scripts/build-java-sdk-docker.js test --profile java17                # 切 maven profile
//
// 首次运行会 docker build 一份 builder 镜像（约 60s）；后续运行 0 build。

const { spawnSync } = require('node:child_process');
const path = require('node:path');

const ROOT = path.join(__dirname, '..');
const SDK_DIR = path.join(ROOT, 'sdk', 'java');

const IMAGE = 'zhcoder-docker-registry.com:8000/builder/java-sdk:0.2.0';
const DOCKERFILE = path.join(SDK_DIR, 'Dockerfile.builder');
const M2_VOLUME = 'coder-java-m2';

const GOALS = {
  compile: ['compile'],
  test: ['test'],
  package: ['package', '-DskipTests'],
  verify: ['verify'],
  deploy: ['deploy', '-DskipTests'],
};

const PROFILES = new Set(['java8', 'java17', 'java21']);

function parseArgs(argv) {
  let goal = 'verify';
  let profile = 'java8';
  const extra = [];
  let passthrough = false;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (passthrough) { extra.push(a); continue; }
    if (a === '--') { passthrough = true; continue; }
    if (a === '--profile') { profile = argv[++i]; continue; }
    if (GOALS[a]) { goal = a; continue; }
    extra.push(a);
  }
  if (!PROFILES.has(profile)) {
    console.error(`unknown profile: ${profile}; valid: ${[...PROFILES].join(', ')}`);
    process.exit(2);
  }
  return { goal, profile, extra };
}

function imageExists() {
  const res = spawnSync('docker', ['image', 'inspect', IMAGE], { stdio: 'ignore' });
  return res.status === 0;
}

function buildImage() {
  console.log(`[build-image] ${IMAGE}`);
  const res = spawnSync('docker', [
    'build',
    '-f', DOCKERFILE,
    '-t', IMAGE,
    SDK_DIR,
  ], { stdio: 'inherit' });
  return res.status === 0;
}

function ensureImage() {
  if (imageExists()) return true;
  console.log(`[info] builder image not found locally, building first time`);
  return buildImage();
}

function runMaven({ goal, profile, extra }) {
  const goalArgs = GOALS[goal];
  const cmd = [
    'run', '--rm',
    '--network', 'host',
    '-v', `${ROOT}:/workspace`,
    '-v', `${M2_VOLUME}:/root/.m2`,
    '-w', '/workspace/sdk/java',
    IMAGE,
    'mvn', `-P${profile}`, ...goalArgs, ...extra,
  ];
  console.log(`[mvn] -P${profile} ${goalArgs.join(' ')}${extra.length ? ' ' + extra.join(' ') : ''}`);
  const res = spawnSync('docker', cmd, { stdio: 'inherit' });
  return res.status === 0;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!ensureImage()) {
    console.error('[fail] could not prepare builder image');
    process.exit(1);
  }
  const ok = runMaven(args);
  console.log('\n=== summary ===');
  console.log(`goal    : ${args.goal}`);
  console.log(`profile : ${args.profile}`);
  console.log(`result  : ${ok ? 'pass' : 'fail'}`);
  process.exit(ok ? 0 : 1);
}

main();
