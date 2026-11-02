#!/usr/bin/env node
/**
 * image-resolver.js — 镜像引用解析（共享模块）
 *
 * 单一职责：把 compose 文件里的 `image:` 引用解析成**最终镜像名列表**。
 *
 * 存在的理由（改造前各脚本各写一套，导致产物不一致）：
 *   - release.js          硬编码 IMAGES 数组（tag 带 v，与 CI 推送规则不一致）
 *   - release-deploy.js   扫 compose，但 `if (img.includes('${')) continue` 会跳过变量形式
 *   - load-images.js      扫 compose，但 tag 正则匹配不了 ${IMAGE_TAG:-x} 插值形式
 *
 * 改造后 compose 里所有 image: 都是 `${VAR}` 形式，三处必须共用本模块，
 * 否则 air-gap 包会静默产出空/缺镜像的产物。
 *
 * 变量插值优先级（与 docker compose 一致）：
 *   1. 进程环境变量
 *   2. --env-file 指定文件（.env 文件）
 *   3. compose 内 `:-` 默认值
 *
 * 用法：
 *   const { scanComposeImages, normalizeImageTag } = require('./image-resolver.js');
 *   const { images, unresolved } = scanComposeImages(composeDir, { envFile, env });
 */

const fs = require('fs');
const path = require('path');

// ── 变量解析 ────────────────────────────────────────────────────────────────

/**
 * 解析 compose 的 ${VAR} / ${VAR:-default} / ${VAR-default} 引用。
 *
 * @param {string} raw        compose 里的原始引用串（可能含多个 ${}）
 * @param {Object} lookup     变量名 → 值 的查找函数（返回 undefined 表示未定义）
 * @returns {{ value: string, unresolved: string[] }}
 *          value 为插值后的字符串；unresolved 为既无环境值也无默认值的变量名
 */
function interpolate(raw, lookup) {
  const unresolved = [];
  // ${VAR:-default} 带默认；${VAR-default} 空值也取默认；${VAR} 无默认
  const value = raw.replace(/\$\{([A-Za-z_][A-Za-z0-9_]*)(?::?-([^}]*))?\}/g, (m, name, def) => {
    const found = lookup(name);
    if (found !== undefined && found !== null && found !== '') return found;
    if (def !== undefined) return def;
    unresolved.push(name);
    return '';
  });
  return { value, unresolved };
}

/**
 * 读取 .env 文件为键值对。忽略注释与空行，支持 `export ` 前缀与引号包裹。
 *
 * @param {string} envFile
 * @returns {Object<string,string>}
 */
function parseEnvFile(envFile) {
  const out = {};
  if (!envFile || !fs.existsSync(envFile)) return out;
  const text = fs.readFileSync(envFile, 'utf-8');
  for (const line of text.split('\n')) {
    const t = line.trim();
    if (!t || t.startsWith('#')) continue;
    const m = t.match(/^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$/);
    if (!m) continue;
    let v = m[2].trim();
    // 去掉行尾注释（仅当值未被引号包裹时）
    if (!/^["']/.test(v)) v = v.replace(/\s+#.*$/, '').trim();
    // 去掉成对引号
    if (/^".*"$/.test(v) || /^'.*'$/.test(v)) v = v.slice(1, -1);
    out[m[1]] = v;
  }
  return out;
}

/**
 * 构造变量查找函数：进程环境变量 > env 文件 > compose 内默认值（由调用方传入的 fallback）。
 *
 * @param {Object} opts
 * @param {string} [opts.envFile]  .env 文件路径
 * @param {Object} [opts.env]      进程环境变量（默认 process.env）
 * @param {Object} [opts.overrides] 最高优先级的显式覆盖值
 * @returns {(name:string)=>string|undefined}
 */
function makeLookup({ envFile, env = process.env, overrides = {} } = {}) {
  const fileVars = parseEnvFile(envFile);
  return (name) => {
    if (Object.prototype.hasOwnProperty.call(overrides, name)) return overrides[name];
    if (env && env[name] !== undefined && env[name] !== '') return env[name];
    if (Object.prototype.hasOwnProperty.call(fileVars, name)) return fileVars[name];
    return undefined;
  };
}

// ── tag 归一 ────────────────────────────────────────────────────────────────

/**
 * 归一镜像 tag：剥掉 git tag 的 v 前缀。
 *
 * 背景：CI 推送镜像用 `export VERSION="${CI_COMMIT_TAG#v}"`（剥 v），
 * registry 里实际存在的是 `micro-server/api-gateway:1.5.0`（不是 :v1.5.0）。
 * 打包脚本若按带 v 的 tag 去 pull，pull 会失败 —— 而旧实现用
 * `ignoreError: true` 静默吞掉，导致 air-gap 包静默缺失全部平台镜像。
 *
 * @param {string} tag
 * @returns {string}
 */
function normalizeImageTag(tag) {
  if (typeof tag !== 'string' || !tag) return tag;
  return tag.replace(/^v(?=\d)/, '');
}

// ── compose 扫描 ────────────────────────────────────────────────────────────

/** compose 文件里 image: 行的匹配（保留缩进外的原样内容） */
const IMAGE_LINE_RE = /^[ \t]+image:[ \t]+(.+)$/gm;

/**
 * 扫描目录下的 compose 文件，解析出最终镜像名列表。
 *
 * @param {string} composeDir   compose 文件所在目录
 * @param {Object} [opts]
 * @param {string[]} [opts.files]  指定文件名（默认扫目录下所有 .yml）
 * @param {string} [opts.envFile]  .env 文件路径
 * @param {Object} [opts.env]      进程环境变量
 * @param {Object} [opts.overrides] 显式覆盖值（最高优先级）
 * @returns {{ images: string[], unresolved: string[] }}
 *          images 为去重排序后的最终镜像名；unresolved 为无法解析的变量名（去重）
 */
function scanComposeImages(composeDir, opts = {}) {
  const { files, envFile, env, overrides } = opts;
  const lookup = makeLookup({ envFile, env, overrides });

  let targets;
  if (files && files.length) {
    targets = files.map(f => path.join(composeDir, f));
  } else {
    targets = fs.existsSync(composeDir)
      ? fs.readdirSync(composeDir)
          .filter(f => f.endsWith('.yml') || f.endsWith('.yaml'))
          .map(f => path.join(composeDir, f))
      : [];
  }

  const imageSet = new Set();
  const unresolvedSet = new Set();

  for (const file of targets) {
    if (!fs.existsSync(file)) continue;
    const text = fs.readFileSync(file, 'utf-8');
    for (const m of text.matchAll(IMAGE_LINE_RE)) {
      const raw = m[1].trim().replace(/^["']|["']$/g, '');
      if (!raw || raw.startsWith('#')) continue;
      const { value, unresolved } = interpolate(raw, lookup);
      for (const u of unresolved) unresolvedSet.add(u);
      const img = value.trim();
      if (!img) continue;
      imageSet.add(img);
    }
  }

  return {
    images: [...imageSet].sort(),
    unresolved: [...unresolvedSet].sort(),
  };
}

module.exports = {
  scanComposeImages,
  interpolate,
  makeLookup,
  normalizeImageTag,
  parseEnvFile,
};
