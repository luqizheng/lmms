#!/usr/bin/env node
/**
 * scripts/transform-compose.js - 将 docker-compose-services.yml 中的
 * build: 块替换为 image: 引用，用于离线分发包。
 *
 * 镜像引用由 .env 驱动（换源只改 .env 一处，无需重新打包）：
 *   image: ${kudi_repos:-<默认仓库>}/<svc>:${IMAGE_TAG:-<打包版本>}
 *
 * 用法：
 *   node scripts/transform-compose.js <compose-file> <version>
 *   node scripts/transform-compose.js <compose-file> <version> <legacy-registry>
 *
 * 示例：
 *   node scripts/transform-compose.js ./infrastructure/docker-compose-services.yml v1.6.0
 *
 * 效果：
 *   config-service:
 *     build:                    →   config-service:
 *       context: ..             →     image: ${kudi_repos:-...}/config-service:${IMAGE_TAG:-1.6.0}
 *       dockerfile: ...         →     (volumes / depends_on 等保留)
 *
 * 不会修改的 build: 块（如第三方镜像、非 services 下的 build）会被忽略。
 *
 * 兼容性：旧的 <registry> 参数保留但**不再用于拼路径**（仓库前缀已由
 * kudi_repos 变量承载），仅打日志提示已废弃，避免旧调用方式直接崩掉。
 */

const fs = require('fs');
const path = require('path');

// 平台镜像的仓库前缀由 .env 的 kudi_repos 变量提供（默认指向内网 registry）。
// compose 里写成 ${kudi_repos:-<默认值>}，用户改 .env 即可换源。
const DEFAULT_KUDI_REPOS = 'zhcoder-docker-registry.com:8000/micro-server';

// ── 服务名 → 镜像名后缀映射 ────────────────────────────────────────────────
// 注意：仓库前缀（micro-server/）已移入 kudi_repos 变量，这里只存后缀。
// 特殊映射保留：
//   - prue_backend 的镜像是 prue-admin-backend（服务名与镜像名不一致）
//   - 两个 outbox-publisher 复用主服务的镜像（同镜像不同 command）
const IMAGE_MAP = {
  'config-service':              'config-service',
  'file-service':                'file-service',
  'api-gateway':                 'api-gateway',
  'token-service':               'token-service',
  'iam-service':                 'iam-service',
  'iam-outbox-publisher':        'iam-service',
  'notification-service':        'notification-service',
  'notification-outbox-publisher':'notification-service',
  'audit-service':               'audit-service',
  'prue_backend':                'prue-admin-backend',
  'mysql-replica':               'mysql-replica',
};

// 方言 → 镜像 tag 后缀（2026-11-03）：PG 变体的镜像在 registry 里是
// `<svc>:1.6.0-pg`（+ `:pg-latest`），不是 `<svc>:1.6.0`。不带后缀会把
// mysql 镜像当成 PG 变体拉下来——运行期才在「连 PG 失败」处暴露。
// 与 `coder build images --dialect pg` 的 tag 规则一致。
const DIALECT_SUFFIX = {
  mysql: '',
  pg: '-pg',
};

// 只有**构建了 PG 变体**的服务才带 -pg 后缀。PG 构建跳过与方言无关的服务
//（api-gateway / docs-site），它们的镜像只有 mysql 版 —— 若也加 -pg，
// compose 会去拉一个不存在的镜像（api-gateway:1.6.0-pg）。
// 与 `coder build images --dialect pg` 的 DB_SERVICES 集合一致。
const PG_VARIANT_SERVICES = new Set([
  'config-service',
  'file-service',
  'token-service',
  'iam-service',
  'iam-outbox-publisher',
  'notification-service',
  'notification-outbox-publisher',
  'audit-service',
  'prue_backend',
]);

// 跳过与 build: 块共存的 image: 行（mysql-replica 同时声明 build: + image:
// 作为本地 tag；transform 后必须丢弃，否则 compose 拿到两个 image: 报错）。
function skipCoexistingImageTag(lines, i) {
  if (i + 1 < lines.length && /^ {4}image:\s+/.test(lines[i + 1])) {
    return i + 1;
  }
  return i;
}

// ── 主逻辑 ─────────────────────────────────────────────────────────────
function main() {
  const args = process.argv.slice(2);
  if (args.length < 2) {
    console.error('用法: node transform-compose.js <compose-file> <version> [<legacy-registry>] [--dialect mysql|pg]');
    console.error('  <compose-file>      compose 文件路径');
    console.error('  <version>           打包版本（可带 v 前缀，内部统一剥 v）');
    console.error('  <legacy-registry>   已废弃：仓库前缀改由 .env 的 kudi_repos 承载');
    console.error('  --dialect pg        产出 PG 变体镜像 tag（<ver>-pg），默认 mysql');
    process.exit(1);
  }

  const [composeFile, rawVersion, legacyRegistry, dialectArg] = args;

  // 镜像 tag 不带 v：CI 推送用 ${CI_COMMIT_TAG#v}（剥 v），registry 里存在的是
  // micro-server/<svc>:1.6.0 而非 :v1.6.0。带 v 去 pull 会失败。
  const version = String(rawVersion).replace(/^v(?=\d)/, '');

  // 方言：pg 变体的镜像 tag 额外带 -pg 后缀（registry 里是 <svc>:1.6.0-pg）。
  // 位置参数：<compose> <version> [legacy-registry] [dialect]；也接受
  // `--dialect pg` 形式（位置参数被 legacy 占位时更清晰）。
  const dialectFlagIdx = args.indexOf('--dialect');
  const dialect = String(
    dialectFlagIdx > 0 ? args[dialectFlagIdx + 1] : dialectArg || 'mysql',
  ).toLowerCase();
  // 注意用 hasOwnProperty 判定：mysql 的后缀是空字符串（falsy），
  // 用 `!DIALECT_SUFFIX[dialect]` 会把合法的 mysql 误判为未知方言。
  if (!Object.prototype.hasOwnProperty.call(DIALECT_SUFFIX, dialect)) {
    console.error(`[transform-compose] ✗ 未知方言: ${dialect}（允许 mysql | pg）`);
    process.exit(1);
  }
  const tagSuffix = DIALECT_SUFFIX[dialect];
  const defaultTag = `${version}${tagSuffix}`;

  // 逐服务的后缀：方言无关的服务（api-gateway）没有 PG 变体镜像，
  // 必须回落到无后缀的 tag，否则会去拉不存在的镜像。
  const tagFor = (serviceName) =>
    dialect === 'pg' && !PG_VARIANT_SERVICES.has(serviceName)
      ? version
      : defaultTag;

  if (legacyRegistry) {
    console.error('[transform-compose] ℹ <registry> 参数已废弃：仓库前缀改由 .env 的 kudi_repos 变量承载');
  }

  const absPath = path.resolve(composeFile);

  if (!fs.existsSync(absPath)) {
    console.error(`[transform-compose] ✗ 文件不存在: ${absPath}`);
    process.exit(1);
  }

  let content = fs.readFileSync(absPath, 'utf-8');
  const lines = content.split('\n');
  const output = [];
  let modified = false;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // 检测服务名行（2 空格缩进 + 服务名 + :）
    // 例如: "  config-service:" 或 "  iam-outbox-publisher:"
    const svcMatch = line.match(/^ {2}([\w-]+):$/);
    if (!svcMatch) {
      output.push(line);
      continue;
    }

    const serviceName = svcMatch[1];
    const imageKey = IMAGE_MAP[serviceName];

    // 先输出服务名行
    output.push(line);

    if (!imageKey) {
      // 不认识的 service，继续
      continue;
    }

    // 看下一行是否是 build: 块
    if (i + 1 >= lines.length) continue;
    const nextLine = lines[i + 1];
    const buildMatch = nextLine.match(/^ {4}build:$/);
    if (!buildMatch) continue;

    // 跳过 build: 行
    i++;

    // 跳过 build 块内所有缩进行（以 6 空格或更多开头）
    // 直到遇到非 build 块内容
    const buildLines = [];
    while (i + 1 < lines.length) {
      const inner = lines[i + 1];
      if (inner.match(/^ {6}/)) {
        // 属于 build 块的内容（context: / dockerfile: / 等）
        buildLines.push(inner);
        i++;
      } else {
        break;
      }
    }

    i = skipCoexistingImageTag(lines, i);

    // 替换为 image: 行（与 build: 同缩进，4 空格）。
    // 仓库前缀走 ${kudi_repos:-<默认>} 变量（.env 可改，无需重新打包）；
    // tag 用 ${IMAGE_TAG:-<version>} 插值：默认值 = 打包版本（不带 v；PG 变体带 -pg），
    // 升级时 upgrade.js 以进程环境变量 IMAGE_TAG 覆盖（compose 插值进程 env 优先于 --env-file）。
    // ⚠️ tagFor(serviceName)：方言无关服务没有 PG 变体镜像 → 回落到无后缀 tag。
    const svcTag = tagFor(serviceName);
    output.push(`    image: \${kudi_repos:-${DEFAULT_KUDI_REPOS}}/${imageKey}:\${IMAGE_TAG:-${svcTag}}`);
    modified = true;
    console.error(`[transform-compose]   ${serviceName}: build → image: \${kudi_repos:-${DEFAULT_KUDI_REPOS}}/${imageKey}:\${IMAGE_TAG:-${svcTag}}`);
  }

  if (!modified) {
    console.error(`[transform-compose] ⚠ 未找到需要替换的 build: 块 — ${absPath}`);
    // 仍然写回（no-op），避免管道断裂
  }

  fs.writeFileSync(absPath, output.join('\n'), 'utf-8');
  console.error(`[transform-compose] ✓ ${absPath}`);
}

main();