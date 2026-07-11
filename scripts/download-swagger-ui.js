#!/usr/bin/env node
// scripts/download-swagger-ui.js
// 把 swagger-ui v5.17.14 zip 预下载到 infrastructure/vendor/，所有 Dockerfile
// 与 cargo test 通过 SWAGGER_UI_DOWNLOAD_URL=file://<path> 离线引用，
// 避免每次 build.rs 触发网络请求（默认直连 github.com，本机 SSL 拦截）。
//
// 用法：
//   node scripts/download-swagger-ui.js          # 已存在跳过；缺失则下载
//   node scripts/download-swagger-ui.js --check  # CI 模式：缺失报错（exit 1）
//   node scripts/download-swagger-ui.js --force  # 强制重下（即使已存在）
//
// zip 元数据集中维护在顶部常量；升版本改 VERSION / ZIP_NAME / EXPECTED_SHA256 即可。

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const https = require('node:https');
const crypto = require('node:crypto');
const { pipeline } = require('node:stream/promises');

const { ROOT } = require('./lib/utils.js');

// ---- 单点维护 -----------------------------------------------------------
const VERSION = 'v5.17.14';
const ZIP_NAME = `swagger-ui-${VERSION}.zip`;
// 公开 zip（gh-proxy.org 转发 github release archive）；团队规则：github 资源走代理
const REMOTE_URL = `https://gh-proxy.org/https://github.com/swagger-api/swagger-ui/archive/refs/tags/${VERSION}.zip`;
// sha256 校验：首次发布时 `sha256sum infrastructure/vendor/<zip>` 填入。
// 升级 swagger-ui 时重算并更新本字段。
const EXPECTED_SHA256 = '481244d0812097b11fbaeef79f71d942b171617f9c9f9514e63acbe13e71ccdc';

const VENDOR_DIR = path.join(ROOT, 'infrastructure', 'vendor');
const DEST = path.join(VENDOR_DIR, ZIP_NAME);

// ---- args ---------------------------------------------------------------
function parseArgs(argv) {
  return {
    check: argv.includes('--check'),
    force: argv.includes('--force'),
  };
}

// ---- helpers ------------------------------------------------------------
function logInfo(msg)  { process.stdout.write(`[swagger-ui] ${msg}\n`); }
function logOk(msg)    { process.stdout.write(`[swagger-ui] \x1b[32m${msg}\x1b[0m\n`); }
function logErr(msg)   { process.stderr.write(`[swagger-ui] \x1b[31m${msg}\x1b[0m\n`); }
function logWarn(msg)  { process.stdout.write(`[swagger-ui] \x1b[33m${msg}\x1b[0m\n`); }

function sha256OfFile(p) {
  return new Promise((resolve, reject) => {
    const h = crypto.createHash('sha256');
    const s = fs.createReadStream(p);
    s.on('error', reject);
    s.on('data', (chunk) => h.update(chunk));
    s.on('end', () => resolve(h.digest('hex')));
  });
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const req = https.get(url, { headers: { 'user-agent': 'coder-micro-setup' } }, (res) => {
      // 跟随 3xx（gh-proxy 在大文件上偶发 302 → github 直连）
      if (res.statusCode && res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        res.resume();
        download(res.headers.location, dest).then(resolve, reject);
        return;
      }
      if (res.statusCode !== 200) {
        res.resume();
        reject(new Error(`HTTP ${res.statusCode} from ${url}`));
        return;
      }
      const total = Number(res.headers['content-length'] || 0);
      let received = 0;
      res.on('data', (chunk) => {
        received += chunk.length;
        if (total) {
          const pct = ((received / total) * 100).toFixed(1);
          process.stdout.write(`\r[swagger-ui] downloading ${VERSION} ... ${pct}% (${(received / 1024 / 1024).toFixed(1)} MB)`);
        }
      });
      const tmp = `${dest}.partial`;
      const ws = fs.createWriteStream(tmp);
      ws.on('error', reject);
      ws.on('finish', () => {
        fs.rename(tmp, dest, (err) => {
          if (err) return reject(err);
          resolve();
        });
      });
      res.on('error', reject);
      res.pipe(ws);
    });
    req.setTimeout(120_000, () => req.destroy(new Error('download timeout (120s)')));
    req.on('error', reject);
  });
}

async function ensureZip({ check, force }) {
  fs.mkdirSync(VENDOR_DIR, { recursive: true });

  if (!force && fs.existsSync(DEST)) {
    const size = fs.statSync(DEST).size;
    if (size < 1024 * 1024) {
      logWarn(`existing file is suspiciously small (${size} bytes), re-downloading`);
      fs.unlinkSync(DEST);
    } else {
      logOk(`found vendor zip: ${path.relative(ROOT, DEST)} (${(size / 1024 / 1024).toFixed(1)} MB)`);
      if (EXPECTED_SHA256) {
        const got = await sha256OfFile(DEST);
        if (got !== EXPECTED_SHA256.toLowerCase()) {
          logErr(`sha256 mismatch: expected=${EXPECTED_SHA256} got=${got}`);
          logErr(`re-run with --force to overwrite`);
          process.exit(2);
        }
      }
      return;
    }
  }

  if (check) {
    logErr(`vendor zip missing: ${path.relative(ROOT, DEST)}`);
    logErr(`run \`pnpm run assets:download\` to fetch it.`);
    process.exit(1);
  }

  logInfo(`downloading ${REMOTE_URL}`);
  const t0 = Date.now();
  await download(REMOTE_URL, DEST);
  const elapsed = ((Date.now() - t0) / 1000).toFixed(1);
  const size = fs.statSync(DEST).size;
  process.stdout.write('\n');
  logOk(`downloaded ${(size / 1024 / 1024).toFixed(1)} MB in ${elapsed}s`);

  if (EXPECTED_SHA256) {
    const got = await sha256OfFile(DEST);
    if (got !== EXPECTED_SHA256.toLowerCase()) {
      logErr(`sha256 mismatch post-download: expected=${EXPECTED_SHA256} got=${got}`);
      logErr(`removing partial file for safety`);
      fs.unlinkSync(DEST);
      process.exit(2);
    }
    logOk('sha256 verified');
  } else {
    logWarn('EXPECTED_SHA256 empty; skipping checksum. Fill it in before shipping.');
    logWarn(`local sha256 for filling: ${await sha256OfFile(DEST)}`);
  }
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  ensureZip(args).catch((err) => {
    logErr(`failed: ${err.message}`);
    process.exit(1);
  });
}

main();