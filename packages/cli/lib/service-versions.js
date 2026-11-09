'use strict';

// service-versions.js —— 从 services/*/Cargo.toml 读平台发布版本
//
// 【为什么是这里】平台镜像 tag 的唯一来源是服务自身的 crate 版本：
//   根 Cargo.toml 的 [workspace.package].version  →  8 个 services/* 用
//   `version.workspace = true` 继承  →  `coder release images` 读出的版本
//   →  镜像 tag（pg 方言加 -pg）→  写进 examples/docker-deploy/.image.env
//
// 【为什么要一致性校验】同一批发布的 8 个服务必须同版本；某个服务单独写死
// 版本会让镜像 tag 参差（有的 1.7.0、有的 1.7.1），下游无法判断该用哪套。
// VERSION_SPLIT 是硬错误：拒绝发布，而不是猜一个版本。
//
// 不依赖 cargo 可执行文件（CI 的 node 容器里没有 cargo），纯文本解析 TOML。

const fs = require('node:fs');
const path = require('node:path');

const { SERVICES, SHARED_DIRS } = require('./image-env.js');

const WORKSPACE_FILE = 'Cargo.toml';

// 取某个 [section] 内的键值（只支持单行 `key = value`；数组/多行值忽略）。
// 返回 Map<key, rawValue>，rawValue 已去引号。
function sectionKeys(tomlText, section) {
  const out = new Map();
  let inside = false;
  for (const line of String(tomlText || '').split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;

    const sec = trimmed.match(/^\[([^\]]+)\]$/);
    if (sec) {
      inside = sec[1] === section;
      continue;
    }
    if (!inside) continue;

    // key 允许带点（version.workspace）；值若是多行/数组（以 [ { 开头）跳过
    const m = trimmed.match(/^([A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*)\s*=\s*(.+)$/);
    if (!m) continue;
    const raw = m[2].trim();
    if (raw.startsWith('[') || raw.startsWith('{')) continue;
    out.set(m[1], raw.replace(/^["'](.*)["']$/, '$1'));
  }
  return out;
}

// 根 workspace 版本（[workspace.package].version）
function readWorkspaceVersion(root) {
  const file = path.join(root, WORKSPACE_FILE);
  if (!fs.existsSync(file)) return null;
  const keys = sectionKeys(fs.readFileSync(file, 'utf8'), 'workspace.package');
  return keys.get('version') || null;
}

// 单个 service 的版本：`version = "x.y.z"` 或 `version.workspace = true`
function readCrateVersion(root, dir) {
  const file = path.join(root, 'services', dir, 'Cargo.toml');
  if (!fs.existsSync(file)) return { dir, file, missing: true };
  const keys = sectionKeys(fs.readFileSync(file, 'utf8'), 'package');
  const crate = keys.get('name') || null;

  if (keys.get('version.workspace') === 'true') {
    return { dir, file, crate, version: readWorkspaceVersion(root), inherited: true };
  }
  const version = keys.get('version');
  return { dir, file, crate, version, inherited: false };
}

// 全部参与发布的服务（镜像服务 + 共享镜像的服务）
function publishedDirs() {
  return [
    ...SERVICES.map((s) => s.dir),
    ...SHARED_DIRS.map((s) => s.dir),
  ];
}

// 读平台版本 + 各服务版本，并做一致性校验。
// 返回：
//   version      平台版本（所有服务一致时的值；不一致时为 null）
//   services     [{ dir, crate, version, inherited, file }]
//   missing      缺失 Cargo.toml 的服务目录
//   splits       版本不一致的服务（dir → version）
//   consistent   version !== null && missing 为空 && splits 为空
function readServiceVersions(root) {
  const dirs = publishedDirs();
  const services = [];
  const missing = [];
  for (const dir of dirs) {
    const r = readCrateVersion(root, dir);
    if (r.missing || !r.version) missing.push({ dir, file: r.file });
    else services.push(r);
  }

  const counts = new Map();
  for (const s of services) counts.set(s.version, (counts.get(s.version) || 0) + 1);

  let version = null;
  let splits = [];
  if (counts.size === 1) {
    version = services[0].version;
  } else if (counts.size > 1) {
    // 多数派作为「期望版本」，少数派列入 splits（便于报错时一眼看出谁没跟上）
    const ranked = [...counts.entries()].sort((a, b) => b[1] - a[1]);
    version = ranked[0][0];
    splits = services.filter((s) => s.version !== version);
  }

  const workspaceVersion = readWorkspaceVersion(root);
  return {
    version,
    workspaceVersion,
    services,
    missing,
    splits,
    consistent: version !== null && missing.length === 0 && splits.length === 0,
  };
}

module.exports = {
  WORKSPACE_FILE,
  sectionKeys,
  readWorkspaceVersion,
  readCrateVersion,
  publishedDirs,
  readServiceVersions,
};
