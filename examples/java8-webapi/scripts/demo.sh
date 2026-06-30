#!/usr/bin/env bash
# scripts/demo.sh — 15-curl walkthrough of the 6 platform capabilities
# (auth + perm + audit + config + notify + file) + 1 业务 (task) demo.
#
# Prereq: demo-webapp (port 7000) is up, and the gateway + 7 Rust services are reachable.
# Override:
#   BASE_URL=http://localhost:7000   (demo-webapp)
#   USER_ID=alice                    (X-User-Id header)

set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:7000}"
USER_ID="${USER_ID:-alice}"

G="\033[0;32m"; Y="\033[0;33m"; N="\033[0m"

step() { printf "\n${Y}==> %s${N}\n" "$1"; }
ok()   { printf "${G}ok${N} %s\n" "$1"; }

# call METHOD PATH [JSON_BODY] [X-Permissions-csv]
call() {
  local method=$1 path=$2 body=${3:-} perms=${4:-}
  local url="$BASE_URL$path"
  local hdr=(-H "X-User-Id: $USER_ID")
  [ -n "$perms" ] && hdr+=(-H "X-Permissions: $perms")
  if [ -n "$body" ]; then
    curl -sS -X "$method" "$url" \
      -H "Content-Type: application/json" \
      "${hdr[@]}" \
      -d "$body"
  else
    curl -sS -X "$method" "$url" "${hdr[@]}"
  fi
}

# open_call METHOD PATH — for whitelisted endpoints (no X-User-Id)
open_call() {
  curl -sS -X "$1" "$BASE_URL$2"
}

# ============================================================
# 1. 认证 (auth) — user-sdk + token-sdk
# ============================================================
step "1)  GET  /demo/auth/jwks     (whitelisted; tokenClient.jwks)"
open_call GET /demo/auth/jwks | head -c 240; echo
ok "auth/jwks"

step "2)  GET  /demo/auth/me       (whitelisted; userClient.getById('me'))"
open_call GET /demo/auth/me | head -c 240; echo
ok "auth/me"

step "3)  POST /demo/auth/refresh  (3-arg body: refresh_token + client_id + client_secret)"
call POST /demo/auth/refresh \
  '{"refresh_token":"sample-refresh-token","client_id":"demo-key","client_secret":"demo-secret"}' \
  | head -c 240; echo
ok "auth/refresh"

# ============================================================
# 2. 权限 (permission) — user-sdk + client-sdk + token-sdk
# ============================================================
step "4)  GET  /demo/perm/public   (whitelisted; local static message)"
open_call GET /demo/perm/public | head -c 200; echo
ok "perm/public"

step "5)  GET  /demo/perm/internal (server-side: clientClient.createRole)"
call GET /demo/perm/internal | head -c 240; echo
ok "perm/internal"

step "6)  GET  /demo/perm/resource (server-side: tokenClient.issueResource('task','42','read',600))"
call GET /demo/perm/resource | head -c 240; echo
ok "perm/resource"

# ============================================================
# 3. 审计 (audit) — audit-service-sdk (manual install)
# ============================================================
step "7)  POST /demo/audit/event   (action/resourceType/resourceId/payload; actor from X-User-Id)"
call POST /demo/audit/event \
  '{"action":"TASK_CREATED","resourceType":"task","resourceId":"42","payload":{"title":"demo"}}' \
  | head -c 240; echo
ok "audit/event"

step "8)  GET  /demo/audit/logs?actor=alice"
call GET '/demo/audit/logs?actor=alice' | head -c 300; echo
ok "audit/logs"

# ============================================================
# 4. 配置中心 (config) — kudi-http-client + ConfigPoller (5s poll)
# ============================================================
step "9)  GET  /demo/config/show?key=foo   (ConfigPoller 5s cache)"
call GET '/demo/config/show?key=foo' | head -c 200; echo
ok "config/show"

# ============================================================
# 5. 通知 (notification) — kudi-http-client
# ============================================================
step "10) POST /demo/notify/task-done      (gateway POST /v1/ntf/notifications)"
call POST /demo/notify/task-done \
  '{"userId":"1001","taskId":"42","title":"Task 42 is done"}' \
  | head -c 200; echo
ok "notify/task-done"

# ============================================================
# 6. 文件 (file) — kudi-http-client
# ============================================================
step "11) POST /demo/file/upload           (multipart → base64 JSON → gateway /v1/file/files)"
echo "hello demo $(date +%s)" > /tmp/demo-upload.txt
UPLOAD=$(curl -sS -X POST "$BASE_URL/demo/file/upload" \
  -H "X-User-Id: $USER_ID" \
  -F "file=@/tmp/demo-upload.txt")
echo "$UPLOAD" | head -c 300; echo
FILE_ID=$(printf '%s' "$UPLOAD" | sed -n 's/.*"fileId":"\([^"]*\)".*/\1/p' | head -1)
[ -n "$FILE_ID" ] || FILE_ID="demo"
ok "file/upload (id=$FILE_ID)"

step "12) GET  /demo/file/$FILE_ID          (binary download, check size)"
curl -sS -X GET "$BASE_URL/demo/file/$FILE_ID" -H "X-User-Id: $USER_ID" -o /tmp/demo-download.bin
ls -la /tmp/demo-download.bin
ok "file/{id}"

# ============================================================
# 7. 业务 (task) — @RequiresPermission AOP demo
# ============================================================
step "13) POST /demo/tasks                  (TASK_CREATE permission required)"
call POST /demo/tasks \
  '{"title":"demo task","description":"by demo.sh"}' \
  "TASK_CREATE" | head -c 300; echo
ok "tasks.create"

step "14) POST /demo/tasks/1/assign         (TASK_ASSIGN permission required; task 1 seeded)"
call POST /demo/tasks/1/assign \
  '{"assigneeId":"bob"}' \
  "TASK_ASSIGN" | head -c 300; echo
ok "tasks/1/assign"

# ============================================================
# 8. 认证收尾 (auth)
# ============================================================
step "15) POST /demo/auth/revoke            (tokenClient.revoke(jti, expires_in))"
call POST /demo/auth/revoke \
  '{"jti":"sample-jti-123","expires_in":0}' | head -c 200; echo
ok "auth/revoke"

printf "\n${G}==> all 15 calls done${N}\n"
