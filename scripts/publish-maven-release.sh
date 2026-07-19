#!/usr/bin/env bash
# Publish Java SDK artifacts to the Reposilite RELEASE repository.
#
# Target repository (id = reposilite-repository-releases):
#   http://192.168.2.12:18181/releases
#
# Two upload modes:
#   curl (default): mvn install -> curl -T upload from ~/.m2 to Reposilite.
#                   Works around Maven Aether HttpClient's Expect:100-continue
#                   incompatibility with Reposilite (causes HTTP 400).
#   mvn  (--mvn):   Direct mvn deploy with -DaltDeploymentRepository.
#                   Use only if Reposilite is upgraded to fix the issue.
#
# Version policy:
#   - Release artifacts MUST NOT carry the -SNAPSHOT suffix.
#   - If the pom version is a -SNAPSHOT, this script aborts with an error.
#   - To release, bump sdk/java/pom.xml <version> to the release version first
#     (e.g. via `mvn -f sdk/java/pom.xml versions:set -DnewVersion=0.3.0
#     versions:commit`), commit, then run this script.
#
# Credentials (two options, pick one):
#   A. Pre-configured ~/.m2/settings.xml with:
#        <server>
#          <id>reposilite-repository-releases</id>
#          <username>...</username>
#          <password>...</password>
#        </server>
#   B. Environment variables MAVEN_USERNAME / MAVEN_PASSWORD, OR --user/--pass
#      flags.
#
# Usage:
#   scripts/publish-maven-release.sh
#   scripts/publish-maven-release.sh --profile java17
#   scripts/publish-maven-release.sh --modules "common,user-service-user-sdk"
#   scripts/publish-maven-release.sh --user admin --pass 's3cret'
#   scripts/publish-maven-release.sh --docker                  # build inside container
#   scripts/publish-maven-release.sh --mvn                     # force mvn deploy mode
#   scripts/publish-maven-release.sh --dry-run                 # verify only, no deploy
#   scripts/publish-maven-release.sh -- -pl user-service-user-sdk -am  # extra mvn args
#
# Environment variables:
#   MAVEN_USERNAME  Reposilite username (alternative to --user)
#   MAVEN_PASSWORD  Reposilite password (alternative to --pass)
#   MAVEN_REPO_ID   Override server id (default: reposilite-repository-releases)
#   MAVEN_REPO_URL  Override repository URL (default: http://192.168.2.12:18181/releases)

set -euo pipefail
cd "$(dirname "$0")/.."

# ---------- defaults ----------
REPO_ID="${MAVEN_REPO_ID:-reposilite-repository-releases}"
REPO_URL="${MAVEN_REPO_URL:-http://192.168.2.12:18181/releases}"
PROFILE="java8"
MODULES=""
USER="${MAVEN_USERNAME:-}"
PASS="${MAVEN_PASSWORD:-}"
DRY_RUN=0
USE_DOCKER=0
USE_MVN=0
EXTRA_MVN_ARGS=()

# ---------- arg parsing ----------
while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)  PROFILE="$2"; shift 2 ;;
    --modules)  MODULES="$2"; shift 2 ;;
    --user)     USER="$2"; shift 2 ;;
    --pass)     PASS="$2"; shift 2 ;;
    --repo-id)  REPO_ID="$2"; shift 2 ;;
    --repo-url) REPO_URL="$2"; shift 2 ;;
    --docker)   USE_DOCKER=1; shift ;;
    --mvn)      USE_MVN=1; shift ;;
    --dry-run)  DRY_RUN=1; shift ;;
    --)         shift; while [[ $# -gt 0 ]]; do EXTRA_MVN_ARGS+=("$1"); shift; done ;;
    -h|--help)
      sed -n '2,42p' "$0"
      exit 0 ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2 ;;
  esac
done

case "${PROFILE}" in
  java8|java17|java21) ;;
  *) echo "ERROR: --profile must be one of: java8, java17, java21" >&2; exit 2 ;;
esac

if ! command -v mvn >/dev/null; then
  echo "ERROR: 'mvn' not found on PATH. Install Maven 3.9+." >&2
  exit 1
fi
if [[ ${USE_MVN} == 0 ]] && ! command -v curl >/dev/null; then
  echo "ERROR: 'curl' not found on PATH (required for default upload mode)." >&2
  exit 1
fi

# ---------- resolve version ----------
VERSION="$(mvn -q -f sdk/java/pom.xml -DforceStdout help:evaluate -Dexpression=project.version 2>/dev/null || true)"
if [[ -z "${VERSION}" ]]; then
  echo "ERROR: could not determine project version from sdk/java/pom.xml." >&2
  exit 2
fi

if [[ "${VERSION}" == *"-SNAPSHOT" ]]; then
  echo "ERROR: sdk/java/pom.xml version is '${VERSION}' (a SNAPSHOT)." >&2
  echo "       The release repository rejects -SNAPSHOT artifacts." >&2
  echo "       Either:" >&2
  echo "         (a) use scripts/publish-maven-snapshot.sh for snapshot deploys, or" >&2
  echo "         (b) bump the pom to a release version first:" >&2
  echo "             mvn -f sdk/java/pom.xml versions:set -DnewVersion=<release-ver> versions:commit" >&2
  exit 3
fi

echo "==> Repository : ${REPO_ID} @ ${REPO_URL}"
echo "==> Version    : ${VERSION}  (from sdk/java/pom.xml)"
echo "==> Profile    : ${PROFILE}"
echo "==> Modules    : ${MODULES:-<all>}"
echo "==> Mode       : $([[ ${USE_MVN} == 1 ]] && echo 'mvn-deploy' || echo 'curl-upload (default)')"
echo "==> Docker     : $([[ ${USE_DOCKER} == 1 ]] && echo yes || echo no)"
echo "==> Dry run    : $([[ ${DRY_RUN} == 1 ]] && echo yes || echo no)"
echo

# ---------- credentials ----------
CURL_AUTH=""
SETTINGS_FILE=""
SETTINGS_ARGS=()
if [[ -n "${USER}" || -n "${PASS}" ]]; then
  if [[ -z "${USER}" || -z "${PASS}" ]]; then
    echo "ERROR: both --user and --pass must be provided." >&2
    exit 2
  fi
  CURL_AUTH="${USER}:${PASS}"
  SETTINGS_FILE="$(mktemp -t reposilite-release-settings.XXXXXX.xml)"
  trap 'rm -f "${SETTINGS_FILE}"' EXIT
  cat > "${SETTINGS_FILE}" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<settings xmlns="http://maven.apache.org/SETTINGS/1.0.0">
  <servers>
    <server>
      <id>${REPO_ID}</id>
      <username>${USER}</username>
      <password>${PASS}</password>
    </server>
  </servers>
</settings>
EOF
  SETTINGS_ARGS=(-s "${SETTINGS_FILE}")
  echo "==> Using temp settings.xml with credentials for server id '${REPO_ID}'."
else
  if [[ -f "${HOME}/.m2/settings.xml" ]] && grep -q "<id>${REPO_ID}</id>" "${HOME}/.m2/settings.xml" 2>/dev/null; then
    echo "==> Using ~/.m2/settings.xml (server id '${REPO_ID}' found)."
    # Extract credentials from settings.xml for curl (best-effort, not parsed in dry-run)
    if [[ ${DRY_RUN} == 0 ]]; then
      SETTINGS_USER=$(python3 -c "
import xml.etree.ElementTree as ET, sys
try:
    tree = ET.parse('${HOME}/.m2/settings.xml')
    ns = {'s': 'http://maven.apache.org/SETTINGS/1.0.0'}
    for srv in tree.findall('.//s:server', ns):
        sid = srv.find('s:id', ns)
        if sid is not None and sid.text == '${REPO_ID}':
            u = srv.find('s:username', ns)
            p = srv.find('s:password', ns)
            if u is not None and p is not None:
                print(f'{u.text}|{p.text}')
                break
except: pass
" 2>/dev/null || true)
      if [[ -n "${SETTINGS_USER}" ]]; then
        CURL_AUTH="${SETTINGS_USER%%|*}:${SETTINGS_USER##*|}"
      fi
    fi
  else
    echo "WARNING: no credentials provided and ~/.m2/settings.xml has no server id '${REPO_ID}'." >&2
  fi
fi

# ---------- dry run ----------
if [[ ${DRY_RUN} == 1 ]]; then
  echo "==> DRY RUN: running clean verify only."
  ( cd sdk/java && mvn -P"${PROFILE}" "${SETTINGS_ARGS[@]}" -DskipTests clean verify )
  echo "==> Dry run OK."
  exit 0
fi

# ---------- mvn deploy mode ----------
if [[ ${USE_MVN} == 1 ]]; then
  ALT_REPO="-DaltDeploymentRepository=${REPO_ID}::default::${REPO_URL}"
  ALT_SNAP="-DaltSnapshotDeploymentRepository=${REPO_ID}::default::${REPO_URL}"
  MVN_ARGS=(-P"${PROFILE}" "${SETTINGS_ARGS[@]}" -DskipTests "${ALT_REPO}" "${ALT_SNAP}")
  [[ -n "${MODULES}" ]] && MVN_ARGS+=(-pl "${MODULES}" -am)
  [[ ${#EXTRA_MVN_ARGS[@]} -gt 0 ]] && MVN_ARGS+=("${EXTRA_MVN_ARGS[@]}")
  echo "[mvn] clean deploy ${MVN_ARGS[*]}"
  ( cd sdk/java && mvn clean deploy "${MVN_ARGS[@]}" )
  echo "==> Release deploy complete (mvn mode)."
  exit 0
fi

# ---------- curl upload mode (default) ----------
# Step 1: mvn install to populate ~/.m2
echo "==> Step 1: mvn install (compile + package to ~/.m2)"
MVN_INSTALL_ARGS=(-P"${PROFILE}" "${SETTINGS_ARGS[@]}" -DskipTests clean install)
[[ -n "${MODULES}" ]] && MVN_INSTALL_ARGS+=(-pl "${MODULES}" -am)
[[ ${#EXTRA_MVN_ARGS[@]} -gt 0 ]] && MVN_INSTALL_ARGS+=("${EXTRA_MVN_ARGS[@]}")
( cd sdk/java && mvn "${MVN_INSTALL_ARGS[@]}" ) || { echo "ERROR: mvn install failed." >&2; exit 4; }

# Step 2: curl -T upload from ~/.m2 to Reposilite
echo
echo "==> Step 2: curl -T upload to ${REPO_URL}"
M2_ROOT="${HOME}/.m2/repository"
# groupId = com.kudi.coder -> path com/kudi/coder
GROUP_PATH="com/kudi/coder"

if [[ -z "${CURL_AUTH}" ]]; then
  echo "ERROR: no curl credentials available. Use --user/--pass or configure ~/.m2/settings.xml." >&2
  exit 5
fi

TOTAL=0; FAIL=0
while IFS= read -r -d '' file; do
  rel="${file#${M2_ROOT}/}"  # e.g. com/kudi/coder/audit-service-sdk/0.3.0/audit-service-sdk-0.3.0.jar
  tmpdir=$(mktemp -d)
  md5sum "$file" | awk '{print $1}' > "$tmpdir/f.md5"
  sha1sum "$file" | awk '{print $1}' > "$tmpdir/f.sha1"
  sha256sum "$file" | awk '{print $1}' > "$tmpdir/f.sha256"

  ok=1
  for suffix in "" ".md5" ".sha1" ".sha256"; do
    src="$file"; [[ -n "$suffix" ]] && src="$tmpdir/f${suffix}"
    remote="${rel}${suffix}"
    # Release repo disallows redeployment: skip files already present.
    if curl -sf -u "${CURL_AUTH}" -o /dev/null "${REPO_URL}/${remote}"; then
      echo "  SKIP ${remote}  (already exists)"
      continue
    fi
    if curl -sf -u "${CURL_AUTH}" -T "$src" -o /dev/null "${REPO_URL}/${remote}"; then
      echo "  OK   ${remote}"
    else
      echo "  FAIL ${remote}"
      ok=0; break
    fi
  done
  rm -rf "$tmpdir"
  TOTAL=$((TOTAL+1))
  [[ $ok -eq 0 ]] && { FAIL=$((FAIL+1)); echo "  -> stopping at ${rel}"; break; }
done < <(find "${M2_ROOT}/${GROUP_PATH}" -path "*/${VERSION}/*" \
  \( -name "*.pom" -o -name "*.jar" \) \
  ! -name "_remote.repositories" \
  ! -name "*.md5" ! -name "*.sha1" ! -name "*.sha256" \
  -print0 | sort -z)

echo
echo "==> Upload complete. Artifacts: ${TOTAL}, Failures: ${FAIL}"
if [[ ${FAIL} -gt 0 ]]; then
  exit 6
fi
echo "==> Release deploy complete (curl mode)."
echo "    Repository: ${REPO_URL}"
echo "    Version   : ${VERSION}"
