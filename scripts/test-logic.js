#!/usr/bin/env node
// WP-TEST-01：无基础设施逻辑测试入口（pnpm run test:logic）。
//
// 目标：在不启动、不连接 MySQL / PostgreSQL / Redis / Consul / RabbitMQ /
// Docker Compose 的前提下，跑完全部 PURE / MEMORY / SQLITE 类测试。
//
// 组成：
//   阶段 A  cargo test --workspace --lib        —— 全部 #[cfg(test)] 单测（PURE 主体）
//   阶段 B  按白名单逐包 --test <file...>        —— PURE / MEMORY / SQLITE 集成测试
//
// 环境纪律：
//   - 显式 unset TEST_MYSQL_HOST / TEST_MYSQL_PORT / TEST_PG_URL / REDIS_URL /
//     DATABASE_URL（防开发机 env 泄漏让"纯"测试意外连真实服务，例如
//     common-db query_macro_test 的 PG 模块、api-gateway blacklist_test）；
//   - SWAGGER_UI_DOWNLOAD_URL 未设置时自动注入 vendor zip（utoipa-swagger-ui
//     build.rs 需要离线 swagger zip，否则编译期直连 GitHub）；
//   - 任何用例失败 → 非 0 退出（不用 skip/ignore 规避断言）。
//
// 白名单治理：
//   node scripts/test-logic.js --check   —— 只校验清单（文件存在性 + 未分类文件警告），不跑测试。

'use strict';

const { spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const repoRoot = path.resolve(__dirname, '..');

// ── PURE / MEMORY / SQLITE 集成测试白名单（与 spec
//    docs/superpowers/specs/2026-11-26-wp-test-01-logic-tests-design.md §2.2 同步）──
// key = cargo 包名（下划线），value = tests/ 下文件名（不含 .rs）。
const PURE_TEST_FILES = {
  // ── libs ──
  digua_authz: ['verify_aud_test'],
  digua_common: ['password_test'],
  digua_common_cache: ['cache_test', 'tiered_cache_test', 'blacklist_service_test'],
  digua_common_config: ['config_test', 'hot_config_test', 'provider_test'],
  // provider_test 用 wiremock 桩 config-service 的 HTTP 导出端点，依赖
  // opt-in feature `config-service`（reqwest）。见 FEATURES 表。
  digua_common_consul: ['registration'],
  digua_common_crypto: ['crypto_test'],
  digua_common_db: [
    'dbpool_test',
    'dialect_test',
    'sql_render_test',
    'scope_test',
    'query_macro_test',
    'sqlite_fixture_test',
  ],
  digua_common_error: ['error_test'],
  digua_common_event: [
    'outbox_sql_test',
    'enqueue_trace_id_test',
    'error_classes',
    'events_codegen',
    'event_trait',
  ],
  digua_common_health: ['health_test'],
  digua_common_http: ['http_test', 'resilient_test', 'resilient_streaming_test'],
  digua_common_jwt: ['cache_test', 'verify_test', 'service_jwt_test'],
  digua_common_log: ['log_test', 'audit_test'],
  digua_common_ratelimit: ['unit_test'],
  digua_common_telemetry: ['current_trace_id_test'],
  digua_common_telemetry_axum: ['access_log_trace_id'],
  digua_common_validator: ['validator_test'],
  digua_jwt_ctx: [
    'extractor_test',
    'principal_test',
    'principal_tenant_test',
    'scope_test',
    'from_token_test',
  ],
  // ── services ──
  api_gateway: [
    'integration_test',
    'template_test',
    'middleware_test',
    'jwt_test',
    'routes_loader_test',
    'tracing_mw_test',
    'resilience_test',
    'router_streaming_test',
    'rate_limit_test', // 3 PURE 用例；2 个 #[ignore]（需真实 Redis）默认不执行
    // blacklist_test 已按 DEC-02 第 9 条移入 INFRA_TEST_FILES（REDIS 类）。
    'blacklist_key_test',
  ],
  audit_service: ['models_test'],
  file_service: ['error_codes_test', 'models_test', 'local_storage_test'],
  iam_service: ['integration_test', 'change_password_test', 'swagger_test'],
  notification_service: [
    'recipient_resolver_test',
    'rate_limit_config_test',
    'iam_client_test',
  ],
  prue_backend: ['menu_config_model_test', 'menu_config_service_test'],
  token_service: ['issuer_test', 'swagger_test'],
};

// 个别包跑白名单测试需要的额外 feature（均为 opt-in、纯 HTTP/嵌入式，不拉任何服务端）。
const PKG_FEATURES = {
  digua_common_config: ['config-service'],
};

// 基础设施类测试文件（MYSQL / REDIS / RABBITMQ / POSTGRES / E2E）——
// 原样保留、照常在 dialect-matrix（GitHub，带 services 容器）与本地
// 开发机执行，但绝不进入 test:logic。用于 --check 的"未分类文件"判定。
const INFRA_TEST_FILES = new Set([
  // api-gateway blacklist_test：REDIS 类（DEC-02 第 9 条）——直连真实
  // Redis（REDIS_URL，缺省 redis://127.0.0.1:6379），常规 CI 不执行；
  // 本地执行方法见该文件头注释。业务逻辑回归由
  // digua_common_cache/tests/blacklist_service_test.rs（MEMORY）承担。
  'blacklist_test',
  // SharedRedisCache（生产 blacklist 适配器）真实 Redis 往返/TTL 验证，
  // REDIS_URL gate + cfg(feature = "redis")，同上仅本地执行。
  'shared_redis_cache_test',
  // common-event（testcontainers rabbitmq/mysql/redis + TEST_PG_URL）
  'consumer_redelivery',
  'ephemeral_subscribe',
  'rabbit_integration',
  'outbox_repo',
  'enqueue_writes_trace_id_to_db',
  'perm_epoch',
  'outbox_pg_test',
  // common-db（TEST_PG_URL）
  'pg_test',
  'pg_migration_test',
  // api-gateway（RED → 已 gate，仍归 infra 类别备案）
  // audit-service
  'login_risk_test',
  'pg_roundtrip_test',
  // file-service
  'tenant_isolation_test',
  // iam-service（spawn_mysql/spawn_redis）
  'internal_membership_test',
  'internal_users_test',
  'tenant_isolation_test',
  'tenant_register_test',
  'tenant_hardening_test',
  'permission_create_test',
  'outbox_archive_test',
  'permission_changed_test',
  'user_roles_assign_test',
  'pg_roundtrip_test',
  // notification-service
  'announcement_publish_test',
  'announcement_recipient_test',
  'membership_projection_test',
  'welcome_consumer_test',
  'integration_test',
  'pg_roundtrip_test',
  // prue_backend
  'pg_roundtrip_test',
  // token-service（spawn_mysql / TEST_PG_URL）
  'migration_marker_test',
  'auth_code_test',
  'oauth_client_test',
  'refresh_marker_test',
  'revoke_test',
  'client_credentials_test',
  'resource_token_auth_test',
  'api_key_test',
  'api_key_auth_test',
  'integration_test',
  'service_jwt_test',
  'service_jwt_grant_test',
  'service_jwt_grant_admin_test',
  'bootstrap_token_admin_test',
  'bootstrap_token_web_test',
  'mysql_grant_unique_test',
  'pg_roundtrip_test',
  'pg_grant_unique_test',
]);

// 必须清除的基础设施 env（存在即可能让"纯"测试意外连真实服务）。
const INFRA_ENV_VARS = [
  'TEST_MYSQL_HOST',
  'TEST_MYSQL_PORT',
  'TEST_PG_URL',
  'REDIS_URL',
  'DATABASE_URL',
];

function run(cmd, args, label, { quietStdout = false } = {}) {
  const started = Date.now();
  console.log(`\n===== [${label}] ${cmd} ${args.join(' ')}`);
  const res = spawnSync(cmd, args, {
    cwd: repoRoot,
    stdio: quietStdout ? ['ignore', 'ignore', 'inherit'] : 'inherit',
    env: process.env,
    shell: process.platform === 'win32',
  });
  const secs = ((Date.now() - started) / 1000).toFixed(1);
  if (res.status !== 0) {
    console.error(`\n[FAIL] ${label} exited ${res.status} after ${secs}s`);
    process.exit(res.status ?? 1);
  }
  console.log(`[OK] ${label} — ${secs}s`);
  return secs;
}

function checkMode() {
  let problems = 0;
  for (const [pkg, files] of Object.entries(PURE_TEST_FILES)) {
    const testsDir = findTestsDir(pkg);
    if (!testsDir) {
      console.error(`[check] missing tests dir for ${pkg}`);
      problems++;
      continue;
    }
    for (const f of files) {
      if (!fs.existsSync(path.join(testsDir, `${f}.rs`))) {
        console.error(`[check] ${pkg}: whitelisted file missing: ${f}.rs`);
        problems++;
      }
    }
  }
  // 未分类文件扫描：既不在 PURE 白名单也不在 infra 排除清单 → 警告（不失败）。
  for (const [pkg, files] of Object.entries(PURE_TEST_FILES)) {
    const testsDir = findTestsDir(pkg);
    if (!testsDir) continue;
    for (const entry of fs.readdirSync(testsDir)) {
      if (!entry.endsWith('.rs')) continue;
      const name = entry.replace(/\.rs$/, '');
      if (!files.includes(name) && !INFRA_TEST_FILES.has(name)) {
        console.warn(
          `[check] UNCLASSIFIED test file: ${pkg}/tests/${entry} — ` +
            'add it to PURE_TEST_FILES or INFRA_TEST_FILES in scripts/test-logic.js'
        );
      }
    }
  }
  console.log(
    problems === 0
      ? '[check] whitelist OK — all whitelisted files exist'
      : `[check] ${problems} problem(s) found`
  );
  process.exit(problems === 0 ? 0 : 1);
}

function findTestsDir(pkg) {
  // cargo 包名（下划线）→ 仓库目录（libs/* 或 services/*，目录名是 kebab/dash）。
  const candidates = [
    path.join(repoRoot, 'libs'),
    path.join(repoRoot, 'services'),
  ];
  for (const base of candidates) {
    for (const dir of fs.readdirSync(base)) {
      const manifest = path.join(base, dir, 'Cargo.toml');
      if (!fs.existsSync(manifest)) continue;
      const content = fs.readFileSync(manifest, 'utf8');
      const m = content.match(/^name\s*=\s*"([^"]+)"/m);
      if (m && m[1] === pkg) {
        const t = path.join(base, dir, 'tests');
        return fs.existsSync(t) ? t : null;
      }
    }
  }
  return null;
}

function main() {
  if (process.argv.includes('--check')) {
    checkMode();
  }

  // 1. 环境纪律：清掉基础设施 env，打印自检证据。
  const removed = [];
  for (const k of INFRA_ENV_VARS) {
    if (process.env[k] !== undefined) {
      removed.push(`${k}=${process.env[k]}`);
      delete process.env[k];
    }
  }
  console.log('[env] infrastructure variables cleared from this process:');
  console.log(removed.length ? `  - ${removed.join('\n  - ')}` : '  - (none were set)');
  for (const k of INFRA_ENV_VARS) {
    if (process.env[k] !== undefined) {
      console.error(`[env] FAILED to clear ${k}`);
      process.exit(1);
    }
  }

  // 2. swagger zip 离线注入（编译需要；CI Linux 无本机 SSL 拦截问题但注入无害）。
  if (!process.env.SWAGGER_UI_DOWNLOAD_URL) {
    const vendor = path.join(repoRoot, 'infrastructure', 'vendor', 'swagger-ui-v5.17.14.zip');
    if (fs.existsSync(vendor)) {
      const url = `file:///${vendor.replace(/\\/g, '/')}`;
      process.env.SWAGGER_UI_DOWNLOAD_URL = url;
      console.log(`[env] SWAGGER_UI_DOWNLOAD_URL auto-set to ${url}`);
    }
  }

  const totalStart = Date.now();

  // 3. 阶段 A：workspace --lib（全部 src 单测）。
  const libJobs = process.env.CARGO_BUILD_JOBS || 4;
  run('cargo', ['test', '--workspace', '--lib', '-j', String(libJobs)], 'A: workspace --lib');

  // 4. 阶段 B：PURE/MEMORY/SQLITE 集成测试白名单（逐包 --test）。
  for (const [pkg, files] of Object.entries(PURE_TEST_FILES)) {
    const args = ['test', '-p', pkg];
    const feats = PKG_FEATURES[pkg];
    if (feats) args.push('--features', feats.join(','));
    for (const f of files) args.push('--test', f);
    run('cargo', args, `B: ${pkg}`);
  }

  const totalSecs = ((Date.now() - totalStart) / 1000).toFixed(1);
  console.log(`\n===== test:logic PASSED — total ${totalSecs}s (no infra touched) =====`);
}

main();
