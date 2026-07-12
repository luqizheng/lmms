#!/usr/bin/env bash
# Publish the three new SDK modules (notification / file / config service-sdk) plus
# audit-service-sdk (existing, kept for symmetry) to the internal Maven repo.
#
# Pre-reqs (env-blocked):
#   1. JDK 17 on PATH (to honor sdk/java's <java.version>17)
#   2. ~/.m2/settings.xml with credentials for <id>kudi-maven</id>:
#        <settings>
#          <servers>
#            <server>
#              <id>kudi-maven</id>
#              <username>${env.MAVEN_USER}</username>
#              <password>${env.MAVEN_PASS}</password>
#            </server>
#          </servers>
#        </settings>
#   3. Network access to http://mvn.kudi.repos (Reposilite)
#
# Bumps sdk/java/{audit,notification,file,config}-service-sdk from 0.1.0/0.3.0-SNAPSHOT
# to whatever this script bumps to (default: 0.3.0). Edit SDK_VERSION below if needed.
#
# Run: scripts/publish-sdks.sh [--dry-run]
#
# This script is intentionally one-shot and idempotent: rerun after a failed attempt
# is safe (Maven deploy is idempotent for unchanged artifacts; snapshots always redeploy).

set -euo pipefail
cd "$(dirname "$0")/.."

SDK_VERSION="${SDK_VERSION:-0.3.0}"
DRY_RUN=0
[[ "${1:-}" == "--dry-run" ]] && DRY_RUN=1

echo "==> SDK publish target version: ${SDK_VERSION}"
echo "==> Profile: java8 (sdk/java/pom.xml sources compile target = 1.8)"
echo

if ! command -v mvn >/dev/null; then
  echo "ERROR: 'mvn' not found on PATH. Install Maven 3.9+ or use a container." >&2
  exit 1
fi

if [[ "${DRY_RUN}" == "1" ]]; then
  echo "==> DRY RUN: skipping actual deploy"
  ( cd sdk/java && mvn -Pjava8 -pl audit-service-sdk,notification-service-sdk,file-service-sdk,config-service-sdk -am -DskipTests verify )
  echo "==> Dry run OK"
  exit 0
fi

# sanity: ensure ~/.m2/settings.xml has the server
if [[ ! -f "${HOME}/.m2/settings.xml" ]] || ! grep -q "<id>kudi-maven</id>" "${HOME}/.m2/settings.xml"; then
  echo "ERROR: ~/.m2/settings.xml missing server id 'kudi-maven'. See header comment in this script." >&2
  exit 2
fi

( cd sdk/java && mvn -Pjava8 \
    -pl audit-service-sdk,notification-service-sdk,file-service-sdk,config-service-sdk \
    -am \
    -DskipTests \
    clean deploy )

echo
echo "==> Done. Next step:"
echo "    1. Update apps/pm/pm-platform-parent/pom.xml:  <kudi-sdk.version>${SDK_VERSION}</kudi-sdk.version>"
echo "    2. Re-run: cd apps/pm && mvn -Pjava17 clean verify"
echo "    3. Run: docker compose -f infrastructure/docker-compose.yml up -d pm"
