// 主机侧执行后端（host runner）。
//
// 2026-11-09 起：**统一走 host 本机 cargo，不再走 WSL2 发行版**。
//
//   - Windows：host 原生 MSVC 工具链（Rust 1.9x + Visual Studio Build Tools
//     提供的 cl.exe）。脚本负责自动定位并注入 VS 开发环境变量
//     （INCLUDE / LIB / PATH），否则 `ring` / `aws-lc-sys` 的 C 依赖会在
//     build script 里报 `D8050 无法加载 c1.dll` / `C1083 stddef.h`。
//   - Linux / macOS：host 原生 gcc/clang，直接本机 cargo。
//
// 与 Docker 构建镜像的分工：Docker（Linux/musl）出的产物用于发布镜像；
// host 侧的产物用于本机跑服务 / 快速验证能否编译通过。

const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');

/** 本地路径 → file:// URL（Windows 盘符路径要转成 /C:/... 形式）。 */
function fileUrl(absPath) {
  const posix = absPath.replace(/\\/g, '/');
  return `file://${posix.startsWith('/') ? '' : '/'}${posix}`;
}

/** 解析 `cmd /c "set"` 的 KEY=VALUE 输出为对象。 */
function parseSetOutput(stdout) {
  const env = {};
  for (const line of String(stdout || '').split(/\r?\n/)) {
    const idx = line.indexOf('=');
    if (idx <= 0) continue;
    env[line.slice(0, idx)] = line.slice(idx + 1);
  }
  return env;
}

/** Visual Studio 安装根目录候选（按版本倒序，覆盖 2022 / 18 等）。 */
function vsInstallRoots() {
  const bases = [
    process.env['ProgramFiles'] && path.join(process.env['ProgramFiles'], 'Microsoft Visual Studio'),
    process.env['ProgramFiles(x86)'] && path.join(process.env['ProgramFiles(x86)'], 'Microsoft Visual Studio'),
  ].filter(Boolean);
  const editions = ['Community', 'Professional', 'Enterprise', 'BuildTools'];
  const roots = [];
  for (const base of bases) {
    let versions = [];
    try {
      versions = fs.readdirSync(base, { withFileTypes: true })
        .filter((d) => d.isDirectory())
        .map((d) => d.name)
        .sort((a, b) => parseInt(b, 10) - parseInt(a, 10));
    } catch {
      continue;
    }
    for (const v of versions) {
      for (const e of editions) {
        roots.push(path.join(base, v, e));
      }
    }
  }
  return roots;
}

/** collectMsvcEnv 的结果缓存——vcvars64.bat + set 约 1s，不能每个 step 重跑。 */
let msvcEnvCache = null;

/**
 * 采集 Visual Studio 开发环境变量（Windows）。
 *
 * 走 `vcvars64.bat && set` 一次性导出——比手工拼 INCLUDE/LIB 更可靠，
 * 因为 Windows SDK 版本目录是随安装变化的。
 *
 * @returns {Object} 需要注入的环境变量（找不到 VS 时返回空对象）
 */
function collectMsvcEnv() {
  if (msvcEnvCache) return msvcEnvCache;
  msvcEnvCache = collectMsvcEnvUncached();
  return msvcEnvCache;
}

function collectMsvcEnvUncached() {
  if (process.platform !== 'win32') return {};
  // 已在 VS 开发环境里（cl.exe 在 PATH）则无需处理。
  const probe = spawnSync('cl.exe', ['?'], { stdio: 'ignore' });
  if (probe.error === undefined && probe.status !== null) return {};

  for (const root of vsInstallRoots()) {
    const vcvars = path.join(root, 'VC', 'Auxiliary', 'Build', 'vcvars64.bat');
    if (!fs.existsSync(vcvars)) continue;
    const res = spawnSync('cmd.exe', ['/s', '/c', `"${vcvars}" >nul 2>&1 && set`], {
      encoding: 'utf8',
    });
    if (res.status !== 0) continue;
    const parsed = parseSetOutput(res.stdout);
    if (!parsed.INCLUDE || !parsed.LIB) continue;
    return {
      INCLUDE: parsed.INCLUDE,
      LIB: parsed.LIB,
      LIBPATH: parsed.LIBPATH || '',
      PATH: parsed.PATH || '',
      WindowsSdkDir: parsed.WindowsSdkDir || '',
      WindowsSdkVerBinPath: parsed.WindowsSdkVerBinPath || '',
      VCToolsInstallDir: parsed.VCToolsInstallDir || '',
    };
  }
  return {};
}

/**
 * 解析执行后端。
 *
 * 2026-11-09 起只保留 host native 一种后端（WSL 路径已下线）。
 *
 * @returns {{backend: string, repoPath: string, env: Object, run: Function}}
 */
function resolveRunner() {
  const msvcEnv = collectMsvcEnv();
  const missingMsvc = process.platform === 'win32' && !msvcEnv.INCLUDE;

  // host native 的 target 目录默认用仓库根的 target/（跨服务共享增量产物）。
  // Windows 下 WSL 时代的 CARGO_TARGET_DIR 指向 Linux 路径已失效，这里忽略。
  const baseEnv = { ...msvcEnv };

  return {
    backend: 'native',
    repoPath: ROOT,
    missingMsvc,
    env: baseEnv,
    run(cmd, args, options = {}) {
      const env = { ...process.env, ...baseEnv, ...(options.env || {}) };
      // reqwest 会读系统代理变量；本机若设了 http_proxy，指向 127.0.0.1 的
      // 数据库请求会被代理走 → 「连接被拒绝」类假失败。补精确 no_proxy 条目。
      const noProxy = ['127.0.0.1', 'localhost', '::1'];
      const currentNoProxy = env.no_proxy || env.NO_PROXY || '';
      const merged = [...new Set([...currentNoProxy.split(','), ...noProxy].filter(Boolean))].join(',');
      env.no_proxy = merged;
      env.NO_PROXY = merged;
      return spawnSync(cmd, args, {
        stdio: options.stdio || 'inherit',
        shell: false,
        cwd: options.cwd || ROOT,
        env,
      });
    },
  };
}

/** 后端里是否装了 cargo-nextest（Docker 路径用的就是它）。 */
function hasNextest(runner) {
  const res = runner.run('cargo', ['nextest', '--version'], { stdio: 'pipe' });
  return res.status === 0;
}

module.exports = {
  ROOT,
  resolveRunner,
  collectMsvcEnv,
  fileUrl,
  hasNextest,
};
