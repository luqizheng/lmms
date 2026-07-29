#!/usr/bin/env node
/**
 * check-java-sdk-drift.js
 *
 * Detects endpoint drift between the Java SDK (*-sdk) and the OpenAPI specs
 * (services/<svc>/openapi.yaml). The Phase 9.5 spec already acknowledged that
 * some Java SDK endpoints reference services that no longer exist or paths
 * that have been renamed since 0.1.0.
 *
 * Drift types reported:
 *   1. SDK path not found in OpenAPI spec → dead reference, will 404 in prod
 *   2. OpenAPI path not wrapped by SDK    → consumer code can't call it
 *
 * Exit codes:
 *   0 — no drift
 *   1 — drift detected (paths reported above)
 *   2 — config error (missing SDK dir / openapi yaml)
 *
 * Usage:
 *   node scripts/check-java-sdk-drift.js
 *   node scripts/check-java-sdk-drift.js --json    # machine-readable output
 */

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.join(__dirname, "..");
const SDK_JAVA_DIR = path.join(REPO_ROOT, "sdk/java");
const SERVICES_DIR = path.join(REPO_ROOT, "services");

const JSON_MODE = process.argv.includes("--json");

// ── Service → SDK mapping ─────────────────────────────────────────────
// Java SDK artifact IDs do not 1:1 map to service directory names.
// Map by the literal path prefix used in the SDK (e.g. `/v1/audit` → audit-service).
const SERVICE_BY_PREFIX = {
  "/v1/audit": "audit-service",
  "/v1/config": "config-service",
  "/v1/file": "file-service",
  "/v1/iam": "iam-service",
  "/v1/notification": "notification-service",
  "/v1/token": "token-service",
  "/v1/tokens": "token-service",
  "/v1/api-keys": "token-service",
};

// ── Helpers ──────────────────────────────────────────────────────────
function readSafe(p) {
  return fs.existsSync(p) ? fs.readFileSync(p, "utf-8") : "";
}

function listSdkModules() {
  if (!fs.existsSync(SDK_JAVA_DIR)) return [];
  return fs
    .readdirSync(SDK_JAVA_DIR, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => path.join(SDK_JAVA_DIR, e.name));
}

/**
 * Extract URL paths referenced by the Java SDK from `*Client.java` files.
 * Strategy:
 *   1. Grep all `"..."` literals that start with `/v1/` inside src/main/java.
 *   2. Detect base-prefix declarations such as `"/v1/iam"` passed to a
 *      constructor and use the prefix to derive sub-paths.
 */
function extractSdkPaths(moduleDir) {
  const paths = new Set();
  const prefixes = new Set();
  const srcRoot = path.join(moduleDir, "src/main/java");
  if (!fs.existsSync(srcRoot)) return { paths, prefixes };

  const walk = (p) => {
    for (const e of fs.readdirSync(p, { withFileTypes: true })) {
      const full = path.join(p, e.name);
      if (e.isDirectory()) walk(full);
      else if (e.isFile() && e.name.endsWith(".java")) {
        const content = readSafe(full);
        // Literal `/v1/<svc>/...` strings — use relative path from prefix
        const litRe = /"(\/v1\/[a-zA-Z][a-zA-Z0-9_\-/]*)"/g;
        let m;
        while ((m = litRe.exec(content)) !== null) {
          paths.add(m[1]);
        }
        // Constructor base prefixes — `super("/v1/iam")` or `new X(cfg, p, "/v1/iam")`
        const prefixRe = /"(\/v1\/(?:iam|audit|file|config|notification|token|tokens|api-keys))"/g;
        while ((m = prefixRe.exec(content)) !== null) {
          prefixes.add(m[1]);
        }
      }
    }
  };
  walk(srcRoot);
  return { paths, prefixes };
}

/**
 * Parse an OpenAPI 3 spec file and return a Set of real `paths` keys.
 * Only `paths.<key>` with at least one HTTP verb is counted.
 */
function extractOpenApiPaths(yamlPath) {
  const content = readSafe(yamlPath);
  const paths = new Set();
  // Simple regex: capture `  /some/path:` immediately followed by an HTTP verb line.
  // Tolerates 2-space (top-level) and 4-space (under x-extension) indentation.
  const re = /^\s{2}(\/[^\s:{}\n]*?):\s*\n(\s+(?:get|post|put|delete|patch|options|head):)/gm;
  let m;
  while ((m = re.exec(content)) !== null) {
    paths.add(m[1]);
  }
  return paths;
}

function serviceToPrefix(serviceName) {
  for (const [prefix, svc] of Object.entries(SERVICE_BY_PREFIX)) {
    if (svc === serviceName) return prefix;
  }
  return null;
}

// ── Main ─────────────────────────────────────────────────────────────
function run() {
  if (!fs.existsSync(SDK_JAVA_DIR)) {
    console.error(`FATAL: SDK directory not found: ${SDK_JAVA_DIR}`);
    process.exit(2);
  }

  const modules = listSdkModules();
  const issues = [];

  // Pre-load OpenAPI specs to avoid re-reading
  const openApiByService = new Map();
  for (const [, service] of Object.entries(SERVICE_BY_PREFIX)) {
    if (openApiByService.has(service)) continue;
    const yaml = path.join(SERVICES_DIR, service, "openapi.yaml");
    if (fs.existsSync(yaml)) {
      openApiByService.set(service, extractOpenApiPaths(yaml));
    }
  }

  for (const moduleDir of modules) {
    const moduleName = path.basename(moduleDir);
    if (moduleName === "common" || moduleName === "kudi-http-client") continue;
    const { paths, prefixes } = extractSdkPaths(moduleDir);

    // Group SDK paths by service prefix
    const byService = new Map();
    for (const sdkPath of [...paths, ...prefixes]) {
      const prefix = Object.keys(SERVICE_BY_PREFIX).find((p) => sdkPath === p || sdkPath.startsWith(p + "/"));
      if (!prefix) continue; // ignore unknown prefix (e.g. /v1/auth/login legacy)
      const svc = SERVICE_BY_PREFIX[prefix];
      if (!byService.has(svc)) byService.set(svc, []);
      // If the path equals the prefix, skip (it's a base URL, not an endpoint)
      if (sdkPath === prefix) continue;
      // Strip prefix and prepend OpenAPI spec root
      // SDK uses `/v1/iam/...` but OpenAPI uses `/iam/...` (gateway strips /v1)
      const subPath = sdkPath.slice(prefix.length);
      const openApiPath = subPath === "" ? "/" : subPath;
      byService.get(svc).push(openApiPath);
    }

    // Check each (SDK path → OpenAPI path) pair
    for (const [svc, sdkPaths] of byService) {
      const openApiPaths = openApiByService.get(svc);
      if (!openApiPaths) {
        issues.push({
          module: moduleName,
          service: svc,
          type: "missing-openapi",
          paths: sdkPaths,
          detail: `No openapi.yaml found for service ${svc}`,
        });
        continue;
      }
      for (const p of sdkPaths) {
        if (!openApiPaths.has(p)) {
          issues.push({
            module: moduleName,
            service: svc,
            type: "sdk-not-in-openapi",
            path: p,
            detail: `SDK calls ${p} but OpenAPI spec does not define it`,
          });
        }
      }

      // Reverse check: OpenAPI paths not wrapped by SDK (informational)
      const sdkPathSet = new Set(sdkPaths);
      const missing = [];
      for (const o of openApiPaths) {
        if (!sdkPathSet.has(o) && !o.includes("{")) {
          // skip path params to avoid false positives
          missing.push(o);
        }
      }
      if (missing.length > 0 && missing.length < openApiPaths.size) {
        // Only report if SDK covers SOME paths but misses others (not full gap)
        issues.push({
          module: moduleName,
          service: svc,
          type: "openapi-not-in-sdk",
          count: missing.length,
          paths: missing.slice(0, 5),
          detail: `OpenAPI defines ${openApiPaths.size} paths; SDK wraps ${sdkPaths.length}; missing ${missing.length} (showing first 5)`,
        });
      }
    }
  }

  // ── Report ──────────────────────────────────────────────────────────
  if (JSON_MODE) {
    console.log(JSON.stringify({ issues }, null, 2));
  } else {
    if (issues.length === 0) {
      console.log("✓ Java SDK drift check passed: no endpoints out of sync with OpenAPI");
      process.exit(0);
    }
    console.error(`✗ Java SDK drift detected: ${issues.length} issue(s)\n`);
    for (const i of issues) {
      if (i.type === "sdk-not-in-openapi") {
        console.error(`  [${i.module}] ${i.service}: ${i.path}  ← ${i.detail}`);
      } else if (i.type === "openapi-not-in-sdk") {
        console.error(`  [${i.module}] ${i.service}: ${i.count} paths not wrapped by SDK (sample: ${i.paths.join(", ")})`);
      } else if (i.type === "missing-openapi") {
        console.error(`  [${i.module}] ${i.service}: ${i.detail}`);
      }
    }
    console.error("\nFix options:");
    console.error("  1. Update SDK to call new OpenAPI path");
    console.error("  2. Revert SDK to legacy endpoint if service has not migrated yet");
    console.error("  3. Mark as deprecated in SDK with @Deprecated annotation + comment");
  }

  // Exit 1 only for hard drift (sdk-not-in-openapi / missing-openapi)
  // "openapi-not-in-sdk" is informational and does not fail CI
  const hardFail = issues.some((i) => i.type === "sdk-not-in-openapi" || i.type === "missing-openapi");
  process.exit(hardFail ? 1 : 0);
}

run();