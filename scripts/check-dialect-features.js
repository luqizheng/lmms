#!/usr/bin/env node
'use strict';

const fs = require('fs');
const path = require('path');

/**
 * 方言 feature 一致性门禁 —— 守护「默认构建产物零变化」这条结构性前提。
 *
 * 已落地形态（方案 3′，宏内 `#[cfg(feature = ...)]` 按**调用方 crate** 求值）：
 *
 *   - 宏内 cfg 与服务 `[features]` 转译必须成对：调用方没定义同名 feature
 *     → 宏分支被全剔除 → E0004/展开成 `()`；
 *   - 依赖声明**不得硬写** `postgres` → 否则默认构建被迫带上 sqlx-postgres
 *     驱动（实测干净构建多 189 秒），破坏「默认构建产物零变化」；
 *   - `libs/common-db` 的 default 必须保持 `["sqlite"]`。
 *
 * 校验项：
 *   1. 宏形态：`match &$pool` 次数 ≥ 宏数量；每个宏有 `_ =>` 兜底臂
 *   2. 服务 `[features]` 段完整（default/mysql/postgres，prue 另有 sqlite）
 *   3. `postgres` feature 转发到 `digua_common_db/postgres`（纯 sqlx 消费者转发到 `sqlx/postgres`）
 *   4. default 不含 postgres；依赖声明不硬写 postgres
 *   5. libs/common-db default == ["sqlite"]
 *
 * 设计：docs/superpowers/specs/2026-11-02-postgresql-service-layer-support-design.md §1.4
 */

const SERVICES = [
  'audit-service',
  'config-service',
  'file-service',
  'iam-service',
  'notification-service',
  'prue_backend',
  'token-service',
  'iam-outbox-publisher',
];

function extractTomlArray(toml, key) {
  const m = toml.match(new RegExp(`^\\s*${key}\\s*=\\s*\\[([^\\]]*)\\]`, 'm'));
  return m ? m[1] : null;
}

function extractFeaturesSection(toml) {
  const m = toml.match(/\[features\]([\s\S]*?)(\n\[|$)/);
  return m ? m[1] : null;
}

function checkQueryRs(errors) {
  const qPath = path.resolve(__dirname, '..', 'libs/common-db/src/query.rs');
  const q = fs.readFileSync(qPath, 'utf8');
  const macros = (q.match(/macro_rules!\s+db_\w+/g) || []).length;
  const guards = (q.match(/match\s+&\$pool/g) || []).length;
  if (guards < macros) {
    errors.push(
      `query.rs: match &\$pool 出现 ${guards} 次，少于宏数量 ${macros}` +
        '（拥有值池不满足 Executor → E0277）',
    );
  }
  const fallbacks = (q.match(/_ => Err\(/g) || []).length;
  if (fallbacks < macros) {
    errors.push(
      `query.rs: 兜底臂 _ => Err(...) 出现 ${fallbacks} 次，少于宏数量 ${macros}` +
        '（Cargo feature 统一会让「库有变体、调用方分支被剔除」，无兜底臂即 E0004）',
    );
  }
}

function checkLibCargoToml(errors) {
  const libToml = fs.readFileSync(
    path.resolve(__dirname, '..', 'libs/common-db/Cargo.toml'),
    'utf8',
  );
  const def = extractTomlArray(libToml, 'default');
  if (def === null) {
    errors.push('libs/common-db: 缺少 default features 声明');
  } else {
    if (!def.includes('sqlite')) errors.push('libs/common-db: default 必须保持含 "sqlite"');
    if (def.includes('postgres')) errors.push('libs/common-db: default 不得含 postgres（D-9）');
  }
}

function checkService(svc, toml, errors) {
  const featuresSection = extractFeaturesSection(toml);
  if (!featuresSection) {
    errors.push(`${svc}: 缺少 [features] 段（宏内 cfg 按调用方 crate 求值，必须定义同名 feature 转译）`);
    return;
  }
  for (const f of ['default', 'mysql', 'postgres']) {
    if (!new RegExp(`^\\s*${f}\\s*=`, 'm').test(featuresSection)) {
      errors.push(`${svc}: [features] 缺少 ${f}`);
    }
  }

  const def = extractTomlArray(featuresSection, 'default');
  if (def !== null && def.includes('postgres')) {
    errors.push(`${svc}: default 不得含 postgres（默认构建产物零变化）`);
  }

  const dep = toml.match(/digua_common_db\s*=\s*\{[^}]*\}/s);
  if (dep) {
    const depFeatures = extractTomlArray(dep[0], 'features') || '';
    if (/["']postgres["']/.test(depFeatures)) {
      errors.push(
        `${svc}: digua_common_db 依赖声明不得硬写 "postgres"` +
          '（方言 feature 只能经 [features] 转译，否则默认构建带上 sqlx-postgres）',
      );
    }
    if (!/postgres\s*=\s*\[[^\]]*"digua_common_db\/postgres"/s.test(featuresSection)) {
      errors.push(`${svc}: postgres feature 必须转发到 digua_common_db/postgres`);
    }
  } else {
    // 纯 sqlx 消费者（如 iam-outbox-publisher）：postgres 必须转发到 sqlx/postgres。
    if (!/postgres\s*=\s*\[[^\]]*"sqlx\/postgres"/s.test(featuresSection)) {
      errors.push(`${svc}: postgres feature 必须转发到 sqlx/postgres`);
    }
  }
}

function main() {
  const errors = [];
  const repoRoot = path.resolve(__dirname, '..');

  checkQueryRs(errors);
  checkLibCargoToml(errors);

  for (const svc of SERVICES) {
    const tomlPath = path.join(repoRoot, 'services', svc, 'Cargo.toml');
    if (!fs.existsSync(tomlPath)) {
      errors.push(`${svc}: Cargo.toml 不存在`);
      continue;
    }
    checkService(svc, fs.readFileSync(tomlPath, 'utf8'), errors);
  }

  if (errors.length > 0) {
    console.error(`FAIL: ${errors.length} 处方言 feature 不一致`);
    errors.forEach((e) => console.error(`  ${e}`));
    process.exit(1);
  }
  console.log(`PASS: ${SERVICES.length} 个服务 + 库的方言 feature 一致性检查通过`);
}

module.exports = { SERVICES, extractTomlArray, extractFeaturesSection, main };

if (require.main === module) {
  main();
}
