#!/usr/bin/env node
/**
 * scripts/transform-compose.js - 将 docker-compose-services.yml 中的
 * build: 块替换为 image: 引用，用于离线分发包。
 *
 * 用法：
 *   node scripts/transform-compose.js <compose-file> <registry> <version>
 *
 * 示例：
 *   node scripts/transform-compose.js ./infrastructure/docker-compose-services.yml \
 *     zhcoder-docker-registry.com:8000 v1.0.0
 *
 * 效果：
 *   config-service:
 *     build:                    →   config-service:
 *       context: ..             →     image: <registry>/micro-server/config-service:<version>
 *       dockerfile: ...         →     (volumes / depends_on 等保留)
 *
 * 不会修改的 build: 块（如第三方镜像、非 services 下的 build）会被忽略。
 */

const fs = require('fs');
const path = require('path');

// ── 服务名 → 镜像名映射 ────────────────────────────────────────────────
// 与 release.sh 中的 IMAGES 数组保持一致
const IMAGE_MAP = {
  'config-service':              'micro-server/config-service',
  'file-service':                'micro-server/file-service',
  'api-gateway':                 'micro-server/api-gateway',
  'token-service':               'micro-server/token-service',
  'iam-service':                 'micro-server/iam-service',
  'iam-outbox-publisher':        'micro-server/iam-service',
  'notification-service':        'micro-server/notification-service',
  'notification-outbox-publisher': 'micro-server/notification-service',
  'audit-service':               'micro-server/audit-service',
  'prue_backend':                'micro-server/prue-admin-backend',
  'frontend':                    'micro-server/frontend',
};

// ── 主逻辑 ─────────────────────────────────────────────────────────────
function main() {
  const args = process.argv.slice(2);
  if (args.length < 3) {
    console.error('用法: node transform-compose.js <compose-file> <registry> <version>');
    process.exit(1);
  }

  const [composeFile, registry, version] = args;
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

    // 替换为 image: 行（与 build: 同缩进，4 空格）
    output.push(`    image: ${registry}/${imageKey}:${version}`);
    modified = true;
    console.error(`[transform-compose]   ${serviceName}: build → image: ${registry}/${imageKey}:${version}`);
  }

  if (!modified) {
    console.error(`[transform-compose] ⚠ 未找到需要替换的 build: 块 — ${absPath}`);
    // 仍然写回（no-op），避免管道断裂
  }

  fs.writeFileSync(absPath, output.join('\n'), 'utf-8');
  console.error(`[transform-compose] ✓ ${absPath}`);
}

main();