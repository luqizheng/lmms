'use strict';

// release-toml.js —— release.toml（平台发布的**镜像/SDK 参数**）的读写
//
// ⚠️ 版本号**不在这里**：平台版本是根 Cargo.toml 的
// [workspace.package].version（见 lib/service-versions.js）。本文件只放
// 方言 / 镜像源 / SDK 目标这类「与版本无关」的参数，避免第二真相源。
//
// 极简 TOML 实现，**不引三方依赖**（本仓 node_modules 精简，内网 npm 源）：
// 只支持本仓用到的子集 —— `[section]` + `key = "value" | true | false | number`。
// 读到不认识的语法直接报错：宁可不发布，也不要静默误读参数。

const fs = require('node:fs');
const path = require('node:path');

const RELEASE_FILE = 'release.toml';

const DEFAULT_DIALECT = 'pg';
const DEFAULT_REGISTRY_HOST = 'zhcoder-docker-registry.com:8000';
const DEFAULT_REGISTRY_REPO = 'micro-server';
const DEFAULT_SDK = {
  rustRegistry: 'kellnr',
  npmRegistry: 'http://npm.kudi.repos',
  mavenRepositoryId: 'reposilite-releases',
};

// ── 解析 ────────────────────────────────────────────────────────────────────
function parseToml(raw) {
  const sections = {};
  const order = [];
  const lines = String(raw || '').split(/\r?\n/);

  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trim();
    if (!trimmed || trimmed.startsWith('#')) continue;

    const sec = trimmed.match(/^\[([A-Za-z0-9_.-]+)\]$/);
    if (sec) {
      const name = sec[1];
      if (!sections[name]) { sections[name] = {}; order.push(name); }
      continue;
    }

    const kv = trimmed.match(/^([A-Za-z0-9_-]+)\s*=\s*(.+)$/);
    if (!kv) throw new Error(`release.toml 第 ${i + 1} 行无法解析: ${trimmed}`);
    const key = kv[1];
    const rawVal = kv[2].trim();
    let value;
    if (/^".*"$/.test(rawVal) || /^'.*'$/.test(rawVal)) value = rawVal.slice(1, -1);
    else if (rawVal === 'true') value = true;
    else if (rawVal === 'false') value = false;
    else if (/^-?\d+(\.\d+)?$/.test(rawVal)) value = Number(rawVal);
    else throw new Error(`release.toml 第 ${i + 1} 行的值类型不支持: ${rawVal}`);

    const current = order[order.length - 1];
    if (!current) throw new Error(`release.toml 第 ${i + 1} 行的键 ${key} 不在任何 [section] 下`);
    sections[current][key] = value;
  }
  return { sections, order };
}

// ── 逐行改写（保留注释 / 排版） ─────────────────────────────────────────────
// 找到目标 section 的 key 行并替换；找不到就插到该 section 末尾；
// section 都没有就追加到文件末尾。
function setTomlValue(raw, section, key, value) {
  const lines = String(raw || '').split(/\r?\n/);
  const rendered = typeof value === 'string' ? `"${value}"` : String(value);
  const line = `${key} = ${rendered}`;

  let sectionStart = -1;
  let sectionEnd = lines.length;
  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trim();
    const sec = trimmed.match(/^\[([A-Za-z0-9_.-]+)\]$/);
    if (!sec) continue;
    if (sectionStart >= 0) { sectionEnd = i; break; }
    if (sec[1] === section) sectionStart = i;
  }

  if (sectionStart >= 0) {
    for (let i = sectionStart + 1; i < sectionEnd; i++) {
      const m = lines[i].match(/^\s*([A-Za-z0-9_-]+)\s*=/);
      if (m && m[1] === key) {
        const out = lines.slice();
        out[i] = line;
        return out.join('\n');
      }
    }
    const out = lines.slice();
    out.splice(sectionEnd, 0, line);
    return out.join('\n');
  }

  const out = lines.slice();
  while (out.length && out[out.length - 1].trim() === '') out.pop();
  out.push('', `[${section}]`, line, '');
  return out.join('\n');
}

// ── 读取 / 校验 ─────────────────────────────────────────────────────────────
function releaseFilePath(root) {
  return path.join(root, RELEASE_FILE);
}

function readReleaseConfig(root) {
  const file = releaseFilePath(root);
  if (!fs.existsSync(file)) {
    const err = new Error(`release.toml 不存在: ${file}`);
    err.code = 'RELEASE_TOML_MISSING';
    throw err;
  }
  const raw = fs.readFileSync(file, 'utf8');
  const { sections } = parseToml(raw);

  const release = sections.release || {};
  const registry = sections.registry || {};
  const sdk = sections.sdk || {};

  const dialect = typeof release.dialect === 'string' ? release.dialect : DEFAULT_DIALECT;
  if (dialect !== 'pg' && dialect !== 'mysql') {
    const err = new Error(`release.toml 的 [release].dialect 非法: ${dialect}（允许 pg | mysql）`);
    err.code = 'RELEASE_DIALECT_INVALID';
    throw err;
  }
  const host = typeof registry.host === 'string' && registry.host ? registry.host : DEFAULT_REGISTRY_HOST;
  const repo = typeof registry.repo === 'string' && registry.repo ? registry.repo : DEFAULT_REGISTRY_REPO;

  return {
    file,
    raw,
    dialect,
    registry: host,
    repo,
    sdk: {
      rustRegistry: sdk.rust_registry === '' ? null : (sdk.rust_registry || DEFAULT_SDK.rustRegistry),
      npmRegistry: sdk.npm_registry === '' ? null : (sdk.npm_registry || DEFAULT_SDK.npmRegistry),
      mavenRepositoryId: sdk.maven_repository_id === '' ? null : (sdk.maven_repository_id || DEFAULT_SDK.mavenRepositoryId),
    },
  };
}

module.exports = {
  RELEASE_FILE,
  DEFAULT_DIALECT,
  DEFAULT_REGISTRY_HOST,
  DEFAULT_REGISTRY_REPO,
  DEFAULT_SDK,
  parseToml,
  setTomlValue,
  readReleaseConfig,
  releaseFilePath,
};
