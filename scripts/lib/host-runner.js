// 主机侧执行后端（host runner）。
//
// 为什么需要这一层：仓库的编译/测试原先只在 Docker（Linux + musl）里跑，
// 迭代成本高。本模块把「在哪儿执行 cargo」这件事收敛到一处：
//
//   - Windows：默认走 WSL2 发行版（debian-rust 等），用发行版里的原生 Linux
//     工具链编译。与 Docker 构建镜像同源（Rust 1.96 + gcc），产物语义一致。
//   - Linux / macOS：直接本机执行 cargo。
//
// 关键约束（2026-10-27 本机实测）：WSL 与 Windows / Docker 之间的 TCP 不通，
// 因此跑在 WSL 里的测试**不能**连 Windows 侧 Docker 暴露的 MySQL/Redis，
// 必须连 WSL 内部的数据库。数据库 host/端口由调用方按后端注入。

const { spawnSync } = require('node:child_process');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');

/** 单引号转义，供 bash -lc 使用。 */
function shQuote(value) {
  const s = String(value);
  return `'${s.replace(/'/g, `'\\''`)}'`;
}

/** 列出本机 WSL 发行版（排除 docker-desktop 这类内部发行版）。 */
function listWslDistros() {
  if (process.platform !== 'win32') return [];
  const res = spawnSync('wsl.exe', ['-l', '-q'], { encoding: 'utf8' });
  if (res.status !== 0) return [];
  return (res.stdout || '')
    .replace(/\0/g, '')
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter(Boolean)
    .filter((s) => !/^docker-desktop(-data)?$/i.test(s));
}

/** 发行版里是否装了 cargo。 */
function distroHasCargo(distro) {
  const res = spawnSync('wsl.exe', ['-d', distro, '--', 'bash', '-lc', 'command -v cargo'], {
    encoding: 'utf8',
  });
  return res.status === 0;
}

/** 挑一个装了 cargo 的发行版；显式指定时不校验则直接返回。 */
function pickWslDistro(explicit) {
  if (explicit) return distroHasCargo(explicit) ? explicit : null;
  const candidates = listWslDistros();
  for (const d of candidates) {
    if (distroHasCargo(d)) return d;
  }
  return candidates[0] || null;
}

/** Windows 路径 → WSL 路径（/mnt/d/...）。优先用 wslpath，失败则手工转换。 */
function toWslPath(winPath) {
  const res = spawnSync('wsl.exe', ['--', 'wslpath', '-a', winPath], { encoding: 'utf8' });
  if (res.status === 0 && res.stdout) return res.stdout.replace(/\0/g, '').trim();
  const m = /^([a-zA-Z]):[\\/](.*)$/.exec(winPath);
  if (!m) return winPath;
  return `/mnt/${m[1].toLowerCase()}/${m[2].replace(/\\/g, '/')}`;
}

/** 发行版的 HOME 目录（WSL 内）。 */
function wslHomeDir(distro) {
  const res = spawnSync('wsl.exe', ['-d', distro, '--', 'bash', '-lc', 'echo $HOME'], {
    encoding: 'utf8',
  });
  const home = (res.stdout || '').replace(/\0/g, '').trim();
  return home || '/root';
}

/**
 * 解析执行后端。
 *
 * @param {{backend?: 'wsl'|'native', distro?: string}} opts
 * @returns {{backend: string, distro: string|null, repoPath: string, run: Function}}
 */
function resolveRunner(opts = {}) {
  const wantBackend = opts.backend || (process.platform === 'win32' ? 'wsl' : 'native');

  if (wantBackend === 'native') {
    return {
      backend: 'native',
      distro: null,
      repoPath: ROOT,
      run(cmd, args, options = {}) {
        return spawnSync(cmd, args, {
          stdio: options.stdio || 'inherit',
          shell: false,
          cwd: options.cwd || ROOT,
          env: options.env ? { ...process.env, ...options.env } : process.env,
        });
      },
    };
  }

  const distro = pickWslDistro(opts.distro);
  if (!distro) {
    throw new Error(
      '未找到可用的 WSL 发行版。装一个带 cargo 的发行版，或用 --backend native 走本机编译。',
    );
  }
  const repoPath = toWslPath(ROOT);
  // WSL 里仓库挂在 /mnt/d（跨文件系统 I/O 慢），target 目录放 WSL 原生盘上更快。
  // Windows 侧的 CARGO_TARGET_DIR（如 d:\compil_world_view_cache\target）在 Linux 下
  // 无效，忽略之；只有显式的 Linux 路径才继承。
  const inheritedTargetDir = process.env.CARGO_TARGET_DIR;
  const inheritedIsWindowsPath =
    !!inheritedTargetDir && /^[a-zA-Z]:[\\/]/.test(inheritedTargetDir);
  const baseEnv = inheritedTargetDir && !inheritedIsWindowsPath
    ? { CARGO_TARGET_DIR: inheritedTargetDir }
    : { CARGO_TARGET_DIR: `${wslHomeDir(distro)}/.cache/cargo-target/low-memory-micro-services` };

  return {
    backend: 'wsl',
    distro,
    repoPath,
    run(cmd, args, options = {}) {
      const envParts = Object.entries({ ...baseEnv, ...(options.env || {}) }).map(
        ([k, v]) => `export ${k}=${shQuote(v)}`,
      );
      const cmdParts = [cmd, ...args].map(shQuote);
      const inner = [
        `cd ${shQuote(options.cwd || repoPath)}`,
        ...envParts,
        cmdParts.join(' '),
      ].join(' && ');
      return spawnSync('wsl.exe', ['-d', distro, '--', 'bash', '-lc', inner], {
        stdio: options.stdio || 'inherit',
        cwd: ROOT,
      });
    },
  };
}

/** 后端里是否装了 cargo-nextest（Docker 路径用的就是它）。 */
function hasNextest(runner) {
  const res = runner.run('cargo', ['nextest', '--version'], { stdio: 'pipe' });
  return res.status === 0;
}

/** 本地路径 → file:// URL（Windows 盘符路径要转成 /C:/... 形式）。 */
function fileUrl(absPath) {
  const posix = absPath.replace(/\\/g, '/');
  return `file://${posix.startsWith('/') ? '' : '/'}${posix}`;
}

module.exports = {
  ROOT,
  resolveRunner,
  listWslDistros,
  pickWslDistro,
  toWslPath,
  fileUrl,
  hasNextest,
};
