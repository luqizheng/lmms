#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');

const SERVICES_DIR = path.join(__dirname, '..', 'services');
const REQUIRED = [
  'Dockerfile',
  'AGENTS.md',
  'README.md',
  'tests/ 目录',
  '/health 端点',
  '/metrics 端点',
  'migrations/ 目录',
  'Swagger 配置（SWAGGER_ENABLED 开关）',
];

function parseExemptions(content) {
  const m = content.match(/## 豁免\s*\n([\s\S]*?)(?:\n##|$)/);
  if (!m) return new Set();
  return new Set(
    m[1]
      .split('\n')
      .map(l => l.replace(/^[-*]\s*/, '').trim())
      .filter(Boolean)
  );
}

function fileExists(p) {
  return fs.existsSync(p);
}

function readSafe(p) {
  return fileExists(p) ? fs.readFileSync(p, 'utf-8') : '';
}

function detectLanguage(serviceDir) {
  const hasCargo = fileExists(path.join(serviceDir, 'Cargo.toml'));
  const hasPom = fileExists(path.join(serviceDir, 'pom.xml'));
  if (hasPom && !hasCargo) return 'java';
  if (hasCargo && !hasPom) return 'rust';
  if (hasCargo && hasPom) return 'mixed';
  return 'unknown';
}

function walkSrcJava(serviceDir) {
  const root = path.join(serviceDir, 'src/main/java');
  const files = [];
  if (!fileExists(root)) return files;
  function recurse(p) {
    for (const e of fs.readdirSync(p, { withFileTypes: true })) {
      const f = path.join(p, e.name);
      if (e.isDirectory()) recurse(f);
      else if (e.isFile() && f.endsWith('.java')) files.push(f);
    }
  }
  recurse(root);
  return files;
}

function walkTestJava(serviceDir) {
  const root = path.join(serviceDir, 'src/test/java');
  const files = [];
  if (!fileExists(root)) return files;
  function recurse(p) {
    for (const e of fs.readdirSync(p, { withFileTypes: true })) {
      const f = path.join(p, e.name);
      if (e.isDirectory()) recurse(f);
      else if (e.isFile() && f.endsWith('.java')) files.push(f);
    }
  }
  recurse(root);
  return files;
}

function hasRouteRust(serviceDir, route) {
  const candidates = [
    path.join(serviceDir, 'src/main.rs'),
    path.join(serviceDir, 'src/routes/mod.rs'),
    path.join(serviceDir, 'src/lib.rs'),
  ];
  const regex = new RegExp(`\\.route\\(\\s*"${route}"`);
  return candidates.some(p => regex.test(readSafe(p)));
}

function hasRouteJava(serviceDir, route) {
  // Spring Boot Actuator exposes /actuator/health + /actuator/prometheus by
  // default when starter-actuator is on the classpath, and the gateway strips
  // /v1/project — so /project/health and /project/metrics are served even
  // without an explicit @GetMapping. We assert the actuator presence + the
  // YAML multiline `management: → ... → include:` exposure block.
  const pom = readSafe(path.join(serviceDir, 'pom.xml'));
  if (!/<artifactId>spring-boot-starter-actuator<\/artifactId>/.test(pom)) return false;
  const exposedSrc = [
    readSafe(path.join(serviceDir, 'src/main/resources/application.yml')),
    readSafe(path.join(serviceDir, 'config/config.yaml')),
  ].join('\n');
  const exposesActuator = /management\s*:[\s\S]*?include\s*:/.test(exposedSrc);
  const exposesTarget = /exposure\s*:[\s\S]*?(health|metrics|prometheus)/.test(exposedSrc);
  if (!exposesActuator || !exposesTarget) return false;
  if (route === '/health') return true;
  if (route === '/metrics') {
    return /<artifactId>(micrometer-registry-prometheus|spring-boot-starter-actuator)<\/artifactId>/.test(pom);
  }
  return false;
}

function hasRoute(serviceDir, route) {
  return hasRouteRust(serviceDir, route) || hasRouteJava(serviceDir, route);
}

function hasSwaggerRust(serviceDir) {
  const cargo = readSafe(path.join(serviceDir, 'Cargo.toml'));
  if (!/utoipa-swagger-ui/.test(cargo)) return false;
  const main = readSafe(path.join(serviceDir, 'src/main.rs'));
  return /SWAGGER_ENABLED/.test(main);
}

function hasSwaggerJava(serviceDir) {
  const pom = readSafe(path.join(serviceDir, 'pom.xml'));
  if (!/<artifactId>springdoc-openapi-starter-webmvc-ui<\/artifactId>/.test(pom)) return false;
  const yml = readSafe(path.join(serviceDir, 'src/main/resources/application.yml'));
  return /springdoc:\s*\n[\s\S]*?api-docs:[\s\S]*?path:\s*\//.test(yml);
}

function hasSwagger(serviceDir) {
  return hasSwaggerRust(serviceDir) || hasSwaggerJava(serviceDir);
}

/**
 * OTel otlp feature 静态守门（iam-service P0 回归防线）。
 *
 * 背景：workspace 用 resolver="2"，服务 Dockerfile 单独 `cargo build -p <svc>`，
 * 依赖闭包内 digua_common_telemetry 的 feature 只由该服务自己的声明决定。
 * iam-service 曾漏写 `features = ["otlp"]` → init_tracing 的 `#[cfg(feature = "otlp")]`
 * exporter 注册分支被整体编译掉 → span 永远到不了 Jaeger。
 *
 * 规则：凡声明 digua_common_telemetry / digua_common_telemetry_axum 依赖的
 * Rust 服务，每个声明必须显式带 otlp feature。豁免在 AGENTS.md `## 豁免` 段
 * 写 `OTel otlp feature`。
 */
function hasOtlpFeatureRust(serviceDir) {
  const cargo = readSafe(path.join(serviceDir, 'Cargo.toml'));
  const deps = cargo.match(/digua_common_telemetry(?:[-_]axum)?\s*=\s*\{([^}]*)\}/g) || [];
  if (deps.length === 0) return true; // 不依赖则不要求
  return deps.every(dep => /features\s*=\s*\[[^\]]*otlp/.test(dep));
}

function hasTests(serviceDir) {
  const lang = detectLanguage(serviceDir);
  if (lang === 'java' || lang === 'mixed') {
    return walkTestJava(serviceDir).length > 0;
  }
  if (fileExists(path.join(serviceDir, 'tests'))) {
    return fs.readdirSync(path.join(serviceDir, 'tests')).some(f => f.endsWith('.rs'));
  }
  return false;
}

function checkComponent(serviceDir, component) {
  switch (component) {
    case 'Dockerfile':
      return fileExists(path.join(serviceDir, 'Dockerfile'));
    case 'AGENTS.md':
      return fileExists(path.join(serviceDir, 'AGENTS.md'));
    case 'README.md':
      return fileExists(path.join(serviceDir, 'README.md'));
    case 'tests/ 目录':
      return hasTests(serviceDir);
    case '/health 端点':
      return hasRoute(serviceDir, '/health');
    case '/metrics 端点':
      return hasRoute(serviceDir, '/metrics');
    case 'migrations/ 目录':
      return fileExists(path.join(serviceDir, 'migrations'));
    case 'Swagger 配置（SWAGGER_ENABLED 开关）':
      return hasSwagger(serviceDir);
    default:
      return false;
  }
}

function main() {
  const services = fs
    .readdirSync(SERVICES_DIR, { withFileTypes: true })
    .filter(d => d.isDirectory())
    .map(d => d.name);

  console.log('=== 服务规范检查报告 ===\n');
  let failed = false;

  for (const svc of services) {
    const svcDir = path.join(SERVICES_DIR, svc);
    const agentsPath = path.join(svcDir, 'AGENTS.md');
    const lang = detectLanguage(svcDir);
    console.log(`\n## ${svc} [lang=${lang}]`);

    if (!fileExists(agentsPath)) {
      console.log('⚠️  缺失规则文件: AGENTS.md');
      failed = true;
      continue;
    }

    const exemptions = parseExemptions(fs.readFileSync(agentsPath, 'utf-8'));

    if (lang === 'rust' && !exemptions.has('OTel otlp feature')) {
      if (hasOtlpFeatureRust(svcDir)) {
        console.log('✓ OTel otlp feature（digua_common_telemetry 显式开启）');
      } else {
        console.log('⚠️  缺失: OTel otlp feature（digua_common_telemetry 依赖须显式 features = ["otlp"]，防 iam-service P0 回归）');
        failed = true;
      }
    }

    for (const comp of REQUIRED) {
      if (exemptions.has(comp)) {
        console.log(`◯ 豁免: ${comp}`);
        continue;
      }
      if (checkComponent(svcDir, comp)) {
        console.log(`✓ ${comp}`);
      } else {
        console.log(`⚠️  缺失: ${comp}`);
        failed = true;
      }
    }
  }

  if (failed) {
    console.log('\n❌ 服务规范检查未通过');
    process.exit(1);
  }
  console.log('\n✅ 服务规范检查通过');
}

main();
