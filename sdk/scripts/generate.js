#!/usr/bin/env node
/**
 * generate.js — 通过 openapi-generator 生成 Java 客户端骨架
 * 实际业务逻辑、TokenCache、异常由手工代码补足（见 sdk/java/*）。
 *
 * 等价于原 sdk/scripts/generate.sh
 */

const { execSync } = require('child_process');
const { existsSync } = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const OPENAPI_DIR = path.join(ROOT, 'openapi');
const OUT_DIR = path.join(ROOT, '.generated');
const JAVA_PKG_BASE = 'com.kudi.coder.generated';

const USER_SPEC = path.join(OPENAPI_DIR, 'user-service.json');
const TOKEN_SPEC = path.join(OPENAPI_DIR, 'token-server.json');

const GENERATOR_JAR = process.env.OPENAPI_GENERATOR_JAR || '/usr/local/bin/openapi-generator-cli.jar';

if (!existsSync(GENERATOR_JAR)) {
  console.error(`openapi-generator-cli.jar not found at ${GENERATOR_JAR}`);
  console.error('Download via: npm i -g @openapitools/openapi-generator-cli');
  process.exit(1);
}

function run(spec, out, extra) {
  const specName = path.basename(spec);
  console.log(`==> generating java for ${specName} -> ${out}`);
  const args = [
    '-jar', GENERATOR_JAR, 'generate',
    '-i', spec,
    '-g', 'java',
    '-o', out,
    '--additional-properties', extra,
    '--skip-validate-spec',
    '--strict-spec=false',
  ];
  execSync(`java ${args.join(' ')}`, { stdio: 'inherit' });
}

run(
  USER_SPEC,
  path.join(OUT_DIR, 'java', 'user-service'),
  `library=okhttp-gson,dateLibrary=java8,sourceFolder=src/main/java,apiPackage=${JAVA_PKG_BASE}.user.api,modelPackage=${JAVA_PKG_BASE}.user.model,invokerPackage=${JAVA_PKG_BASE}.user.invoker,artifactId=user-service-generated`,
);

run(
  TOKEN_SPEC,
  path.join(OUT_DIR, 'java', 'token-server'),
  `library=okhttp-gson,dateLibrary=java8,sourceFolder=src/main/java,apiPackage=${JAVA_PKG_BASE}.token.api,modelPackage=${JAVA_PKG_BASE}.token.model,invokerPackage=${JAVA_PKG_BASE}.token.invoker,artifactId=token-server-generated`,
);

console.log(`==> done. outputs at ${OUT_DIR}`);
