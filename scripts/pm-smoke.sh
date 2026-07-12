#!/usr/bin/env bash
# apps/pm smoke test. Probes liveness/readiness, then runs a minimal login -> create project
# -> list -> archive -> list flow to validate the wiring end-to-end.
#
# Pre-reqs (env-blocked):
#   1. apps/pm reachable at ${PM_BASE:-http://localhost:8088} — bring up via one of:
#        - cd apps/pm && mvn -Pjava17 spring-boot:run
#        - docker compose -f infrastructure/docker-compose.yml up -d pm
#   2. MySQL reachable at ${MYSQL_URL:-jdbc:mysql://localhost:3306/superpowers} with
#      Flyway V1 already applied (this happens automatically on first boot).
#   3. Static dev bearer already issued — ${PM_JWT} env-var OR use --login flag (interactive).
#
# Run: scripts/pm-smoke.sh [--login]
# Exit code = number of failed checks (0 = all green).

set -uo pipefail
PM_BASE="${PM_BASE:-http://localhost:8088}"
PM_JWT_DEFAULT="dev-token-not-real"   # placeholder; --login performs a real login if available
INTERACTIVE=0
[[ "${1:-}" == "--login" ]] && INTERACTIVE=1

PASS=0
FAIL=0

ok()   { printf "  \033[32m✓\033[0m %s\n" "$1"; PASS=$((PASS + 1)); }
bad()  { printf "  \033[31m✗\033[0m %s — %s\n" "$1" "$2"; FAIL=$((FAIL + 1)); }

heading() { printf "\n\033[1m== %s ==\033[0m\n" "$1"; }

# ---- 1. liveness ----
heading "liveness"
code=$(curl -s -o /dev/null -w '%{http_code}' "${PM_BASE}/pm-api/v1/health" || echo "000")
if [[ "$code" == "200" ]]; then ok "GET /health 200"; else bad "GET /health" "code=$code"; fi

# ---- 2. readiness (DB ping) ----
heading "readiness"
code=$(curl -s -o /dev/null -w '%{http_code}' "${PM_BASE}/pm-api/v1/health/ready" || echo "000")
if [[ "$code" == "200" ]]; then ok "GET /health/ready 200"; else bad "GET /health/ready" "code=$code (DB unreachable?)"; fi

# ---- 3. JWT (use --login to bootstrap, else mint from PM_JWT_SECRET) ----
heading "auth"

# B64url-encode stdin (strip padding, no newlines)
b64url() { openssl base64 -e -A | tr -d '\n' | tr '+/' '-_' | tr -d '='; }

# Mint an HS256 JWT using the same secret apps/pm's JwtConfig reads.
# Mirrors JwtConfig defaults (issuer=pm-platform) and JwtAuthFilter's claims
# (sub as Long-parseable, optional `uname`).
# Reference: apps/pm/pm-app-bootstrap/.../JwtConfig.java + JwtAuthFilter.java
mint_test_jwt() {
  local secret="$1"
  local now; now=$(date +%s)
  local exp=$((now + 3600))
  local header='{"alg":"HS256","typ":"JWT"}'
  local payload="{\"sub\":\"1\",\"uname\":\"dev\",\"iat\":${now},\"exp\":${exp},\"iss\":\"pm-platform\"}"
  local h; h=$(printf '%s' "$header" | b64url)
  local p; p=$(printf '%s' "$payload" | b64url)
  local sig; sig=$(printf '%s.%s' "$h" "$p" | openssl dgst -binary -sha256 -hmac "$secret" | b64url)
  printf '%s.%s.%s' "$h" "$p" "$sig"
}

JWT="${PM_JWT:-}"
if [[ "$INTERACTIVE" == "1" ]]; then
  read -r -p "    pm username: " PM_USER
  read -r -s -p "    pm password: " PM_PASS
  echo
  JWT=$(curl -fsS -X POST "${PM_BASE}/pm-api/v1/auth/login" \
        -H "Content-Type: application/json" \
        -d "{\"username\":\"${PM_USER}\",\"password\":\"${PM_PASS}\"}" \
        | grep -oE '"data":"[^"]+"' | sed 's/"data":"//;s/"$//') || JWT=""
fi
if [[ -z "${JWT:-}" ]]; then
  if [[ -n "${PM_JWT_SECRET:-}" ]]; then
    JWT=$(mint_test_jwt "${PM_JWT_SECRET}")
    ok "JWT bearer minted from PM_JWT_SECRET (length=${#JWT})"
  else
    bad "JWT bearer" "set PM_JWT or PM_JWT_SECRET env-var, or use --login"
  fi
elif [[ "${JWT}" != "$PM_JWT_DEFAULT" ]]; then
  ok "JWT bearer acquired (length=${#JWT})"
fi

hdr_auth=()
[[ -n "${JWT:-}" && "${JWT}" != "$PM_JWT_DEFAULT" ]] && hdr_auth=(-H "Authorization: Bearer ${JWT}")

# ---- 4. CRUD round-trip on /projects ----
heading "project CRUD"
suffix="$(date +%s)"
code=$(curl -s -o /tmp/pm-create.$$ -w '%{http_code}' \
  -X POST "${PM_BASE}/pm-api/v1/projects" \
  -H "Content-Type: application/json" \
  "${hdr_auth[@]}" \
  -d "{\"code\":\"smoke-${suffix}\",\"name\":\"smoke ${suffix}\"}" || echo 000)
if [[ "$code" == "200" || "$code" == "201" ]]; then
  ok "POST /projects -> $code"
else
  bad "POST /projects" "code=$code"
fi
PROJECT_ID=$(grep -oE '"id":[0-9]+' /tmp/pm-create.$$ 2>/dev/null | head -1 | cut -d: -f2 || echo "")
[[ -n "$PROJECT_ID" ]] && ok "created project id=$PROJECT_ID"

code=$(curl -s -o /tmp/pm-list.$$ -w '%{http_code}' \
  "${PM_BASE}/pm-api/v1/projects?page=1&size=10" "${hdr_auth[@]}" || echo 000)
if [[ "$code" == "200" ]]; then ok "GET /projects -> 200"; else bad "GET /projects" "code=$code"; fi

if [[ -n "$PROJECT_ID" ]]; then
  code=$(curl -s -o /dev/null -w '%{http_code}' \
    -X PATCH "${PM_BASE}/pm-api/v1/projects/${PROJECT_ID}" \
    -H "Content-Type: application/json" \
    "${hdr_auth[@]}" \
    -d '{"status":"archived"}' || echo 000)
  if [[ "$code" == "200" ]]; then ok "PATCH /projects/$PROJECT_ID -> 200"; else bad "PATCH" "code=$code"; fi

  code=$(curl -s -o /dev/null -w '%{http_code}' \
    -X DELETE "${PM_BASE}/pm-api/v1/projects/${PROJECT_ID}" \
    "${hdr_auth[@]}" || echo 000)
  if [[ "$code" == "204" || "$code" == "200" ]]; then ok "DELETE /projects/$PROJECT_ID -> $code"; else bad "DELETE" "code=$code"; fi
fi
rm -f /tmp/pm-create.$$ /tmp/pm-list.$$

# ---- 5. KB CRUD quick pass ----
heading "kb CRUD"
code=$(curl -s -o /tmp/pm-space.$$ -w '%{http_code}' \
  -X POST "${PM_BASE}/pm-api/v1/spaces" \
  -H "Content-Type: application/json" \
  "${hdr_auth[@]}" \
  -d "{\"name\":\"smoke-${suffix}\",\"visibility\":\"private\"}" || echo 000)
[[ "$code" == "200" || "$code" == "201" ]] && ok "POST /spaces -> $code" || bad "POST /spaces" "code=$code"
SPACE_ID=$(grep -oE '"id":[0-9]+' /tmp/pm-space.$$ 2>/dev/null | head -1 | cut -d: -f2 || echo "")
[[ -n "$SPACE_ID" ]] && ok "created space id=$SPACE_ID"
rm -f /tmp/pm-space.$$

# ---- 6. LLM endpoint should always respond (even if disabled) ----
heading "llm"
code=$(curl -s -o /tmp/pm-prov.$$ -w '%{http_code}' \
  "${PM_BASE}/pm-api/v1/llm/providers" "${hdr_auth[@]}" || echo 000)
[[ "$code" == "200" ]] && ok "GET /llm/providers -> 200" || bad "GET /llm/providers" "code=$code"
rm -f /tmp/pm-prov.$$

printf "\n\033[1mSummary:\033[0m %d passed, %d failed\n" "$PASS" "$FAIL"
exit "$FAIL"
