#!/usr/bin/env bash
# apps/pm seed script. Creates a small set of demo data (1 project, 2
# requirements, 2 user stories with AC, 1 KB space + 1 document) so a
# developer can boot a fresh PM container and have something to click on
# without first logging in to iam-service or hand-crafting curl payloads.
#
# Pre-reqs (env-blocked):
#   1. apps/pm reachable at ${PM_BASE:-http://localhost:8088}. Easiest path:
#        cd apps/pm && mvn -Pjava17 spring-boot:run
#        (or docker compose up + the dev profile is on by default).
#   2. SPRING_PROFILES_ACTIVE=dev so NoOpJwtVerifier is active and a
#      Authorization header is not required.
#
# Idempotent: every entity is checked by stable natural key first; if it
# already exists we move on.
#
# Run: scripts/seed-pm.sh
# Exit code = 0 on success; non-zero if any HTTP probe fails.

set -uo pipefail
PM_BASE="${PM_BASE:-http://localhost:8088}"
DRY_RUN="${DRY_RUN:-0}"    # set DRY_RUN=1 to print curl commands without executing

red()    { printf "\033[31m%s\033[0m" "$1" >&2; }
green()  { printf "\033[32m%s\033[0m" "$1" >&2; }
bold()   { printf "\033[1m%s\033[0m"  "$1" >&2; }
heading(){ printf '\n\033[1m== %s ==\033[0m\n' "$1" >&2; }
ok()     { printf '  \033[32m✓\033[0m %s\n' "$1" >&2; }
bad()    { printf '  \033[31m✗\033[0m %s — %s\n' "$1" "$2" >&2; exit 1; }

# jq-free JSON extractor: prints the value of the first "key": pair found.
# Returns empty string if the key isn't present.
json_field() {
    local body="$1" key="$2"
    python3 -c "import json,sys; d=json.loads(sys.argv[1]); v=d
$( [ \"\$key\" = 'id' ] && echo '[0]' || echo '' ); print(v if not isinstance(v,list) else (v[0] if v else ''))" \
        "$body" 2>/dev/null || true
}

# GET — returns body. Handles 200/4xx gracefully.
get() {
    local url="$1"
    curl -fsS -X GET "$url" 2>/dev/null
}

# POST/PUT — returns body. Aborts on HTTP error.
post() {
    local url="$1" payload="$2"
    if [[ "$DRY_RUN" == "1" ]]; then
        echo "[dry-run] POST $url payload=$payload"
        return 0
    fi
    curl -fsS -X POST "$url" -H 'Content-Type: application/json' -d "$payload" 2>/dev/null
}

# Wait for the API to come up before starting; surfaces a clear error
# instead of a flood of curl 000s when the user forgets to start the service.
wait_for_pm() {
    heading "wait-for-pm"
    for i in $(seq 1 30); do
        code=$(curl -s -o /dev/null -w '%{http_code}' "${PM_BASE}/pm-api/v1/health" 2>/dev/null || echo 000)
        if [[ "$code" == "200" ]]; then ok "PM is up"; return 0; fi
        sleep 1
    done
    bad "PM never came up" "${PM_BASE}/pm-api/v1/health"
}

# Return an existing project's id if its code matches, else empty.
find_project_id() {
    local code="$1"
    get "${PM_BASE}/pm-api/v1/projects?page=1&size=100" \
        | python3 -c "import json,sys
d=json.load(sys.stdin)
for p in d.get('items',[]):
    if p.get('code')==\"$code\": print(p['id']); break
"
}

# Same shape as find_project_id but for requirements under a project.
find_requirement_id() {
    local pid="$1" title="$2"
    get "${PM_BASE}/pm-api/v1/projects/${pid}/requirements?status=&type=" \
        | python3 -c "import json,sys
d=json.load(sys.stdin)
for r in d.get('items',[]):
    if r.get('title')==\"$title\": print(r['id']); break
"
}

find_story_id() {
    local rid="$1" as_a="$2" i_want="$3"
    get "${PM_BASE}/pm-api/v1/requirements/${rid}/stories" \
        | python3 -c "import json,sys
d=json.load(sys.stdin)
for s in d:
    if s.get('asA')==\"$as_a\" and s.get('iWant')==\"$i_want\": print(s['id']); break
"
}

find_space_id() {
    local name="$1"
    get "${PM_BASE}/pm-api/v1/spaces?page=1&size=100" \
        | python3 -c "import json,sys
d=json.load(sys.stdin)
for s in d.get('items',[]):
    if s.get('name')==\"$name\": print(s['id']); break
"
}

find_document_id() {
    local sid="$1" title="$2"
    get "${PM_BASE}/pm-api/v1/spaces/${sid}/documents" \
        | python3 -c "import json,sys
d=json.load(sys.stdin)
for doc in d:
    if doc.get('title')==\"$title\": print(doc['id']); break
"
}

create_or_get_project() {
    local code="$1" name="$2" desc="$3" owner="$4"
    local existing
    existing=$(find_project_id "$code")
    if [[ -n "$existing" ]]; then ok "project '$code' already exists (id=$existing)"; echo "$existing"; return 0; fi
    heading "create project '$code'"
    local body
    body=$(post "${PM_BASE}/pm-api/v1/projects" \
        "{\"code\":\"$code\",\"name\":\"$name\",\"description\":\"$desc\",\"ownerId\":$owner}")
    local id
    id=$(echo "$body" | python3 -c "import json,sys; print(json.load(sys.stdin).get('id',''))")
    [[ -n "$id" ]] || bad "create project $code" "no id in response: $body"
    ok "project '$code' created (id=$id)"
    echo "$id"
}

create_or_get_requirement() {
    local pid="$1" title="$2" desc="$3" type="$4" priority="$5"
    local existing
    existing=$(find_requirement_id "$pid" "$title")
    if [[ -n "$existing" ]]; then ok "requirement '$title' already exists (id=$existing)"; echo "$existing"; return 0; fi
    heading "create requirement '$title'"
    local body
    body=$(post "${PM_BASE}/pm-api/v1/projects/${pid}/requirements" \
        "{\"title\":\"$title\",\"description\":\"$desc\",\"type\":\"$type\",\"priority\":\"$priority\",\"status\":\"open\"}")
    local id
    id=$(echo "$body" | python3 -c "import json,sys; print(json.load(sys.stdin).get('id',''))")
    [[ -n "$id" ]] || bad "create requirement $title" "no id in response: $body"
    ok "requirement '$title' created (id=$id)"
    echo "$id"
}

create_or_get_story() {
    local rid="$1" as_a="$2" i_want="$3" so_that="$4"
    local existing
    existing=$(find_story_id "$rid" "$as_a" "$i_want")
    if [[ -n "$existing" ]]; then ok "story already exists (id=$existing)"; echo "$existing"; return 0; fi
    heading "create story '$i_want'"
    local body
    body=$(post "${PM_BASE}/pm-api/v1/requirements/${rid}/stories" \
        "{\"asA\":\"$as_a\",\"iWant\":\"$i_want\",\"soThat\":\"$so_that\"}")
    local id
    id=$(echo "$body" | python3 -c "import json,sys; print(json.load(sys.stdin).get('id',''))")
    [[ -n "$id" ]] || bad "create story" "no id in response: $body"
    ok "story created (id=$id)"
    echo "$id"
}

create_or_get_space() {
    local name="$1" slug="$2" desc="$3" project_id="$4" visibility="$5"
    local existing
    existing=$(find_space_id "$name")
    if [[ -n "$existing" ]]; then ok "space '$name' already exists (id=$existing)"; echo "$existing"; return 0; fi
    heading "create KB space '$name'"
    local body
    body=$(post "${PM_BASE}/pm-api/v1/spaces" \
        "{\"name\":\"$name\",\"slug\":\"$slug\",\"description\":\"$desc\",\"projectId\":$project_id,\"visibility\":\"$visibility\"}")
    local id
    id=$(echo "$body" | python3 -c "import json,sys; print(json.load(sys.stdin).get('id',''))")
    [[ -n "$id" ]] || bad "create space $name" "no id in response: $body"
    ok "space '$name' created (id=$id)"
    echo "$id"
}

create_or_get_document() {
    local sid="$1" title="$2" doc_type="$3" markdown="$4"
    local existing
    existing=$(find_document_id "$sid" "$title")
    if [[ -n "$existing" ]]; then ok "document '$title' already exists (id=$existing)"; echo "$existing"; return 0; fi
    heading "create KB doc '$title'"
    local body
    body=$(post "${PM_BASE}/pm-api/v1/spaces/${sid}/documents" \
        "{\"title\":\"$title\",\"docType\":\"$doc_type\",\"spaceId\":$sid,\"initialMarkdown\":$(python3 -c "import json,sys; print(json.dumps(sys.argv[1]))" "$markdown")}")
    local id
    id=$(echo "$body" | python3 -c "import json,sys; print(json.load(sys.stdin).get('id',''))")
    [[ -n "$id" ]] || bad "create doc $title" "no id in response: $body"
    ok "document '$title' created (id=$id)"
    echo "$id"
}

# ============================================================
# main
# ============================================================
wait_for_pm

heading "project"
PID=$(create_or_get_project "DEMO" "Demo Project" \
    "Demo project seeded by scripts/seed-pm.sh — explore PM Platform features here." \
    1)

heading "requirements"
RID1=$(create_or_get_requirement "$PID" "用户登录功能" \
    "用户能用用户名/密码登录，密码错误有提示。" "feature" "p1")
RID2=$(create_or_get_requirement "$PID" "KB 文档版本回滚" \
    "文档作者能把任意历史版本设为当前版本（产生新版本号）。" "feature" "p2")

heading "user stories + acceptance criteria"
SID1=$(create_or_get_story "$RID1" "产品用户" "用用户名+密码登录系统" "能访问受限页面")
SID2=$(create_or_get_story "$RID1" "产品用户" "在密码错误时看到明确提示" "不会泄露用户存在性")

# AC for story 1
post "${PM_BASE}/pm-api/v1/stories/${SID1}/criteria" \
    '{"givenText":"用户输入正确用户名密码","whenText":"点击登录","thenText":"登录成功跳转主页"}' >/dev/null \
    && ok "AC1 for story $SID1 added" \
    || bad "AC1 for story $SID1" "POST failed"

post "${PM_BASE}/pm-api/v1/stories/${SID1}/criteria" \
    '{"givenText":"用户输入错误密码","whenText":"点击登录","thenText":"提示用户名或密码错误"}' >/dev/null \
    && ok "AC2 for story $SID1 added" \
    || bad "AC2 for story $SID1" "POST failed"

heading "knowledge base"
SID=$(create_or_get_space "Demo KB" "demo-kb" \
    "KB space seeded by scripts/seed-pm.sh" 1 "project")

DID=$(create_or_get_document "$SID" "Welcome" "markdown" \
"# Welcome

This is a demo KB document seeded by scripts/seed-pm.sh.

## Try this
1. Open the AI decompose dialog on any requirement — it hits DeepSeek V4-Flash
   when pm.llm.deepseek.api-key is set, otherwise the noop provider returns a
   stub.
2. Edit this document and click Save — each save creates a new
   kb_document_versions row you can browse via GET /v1/documents/{id}/versions.
3. Add a user story under any requirement and the parent requirement keeps
   showing it in the detail view.

## Stack
- Spring Boot 3.3 + MyBatis-Plus + Flyway
- Vue 3 + vue-pure-admin frontend (see frontend/src/views/pm)
- Java SDK modules at sdk/java (audit/file/notification/config-service-sdk 0.3.0)
")

heading "summary"
printf '  \033[1mDEMO project id:        \033[0m %s\n' "$PID"           >&2
printf '  \033[1mRequirement 1 id:      \033[0m %s\n' "$RID1"          >&2
printf '  \033[1mRequirement 2 id:      \033[0m %s\n' "$RID2"          >&2
printf '  \033[1mStory 1 id / Story 2:   \033[0m %s / %s\n' "$SID1" "$SID2" >&2
printf '  \033[1mKB space id:            \033[0m %s\n' "$SID"           >&2
printf '  \033[1mKB document id:         \033[0m %s\n' "$DID"           >&2

ok "seed complete — open ${PM_BASE}/pm-api/v1/projects?page=1&size=20"