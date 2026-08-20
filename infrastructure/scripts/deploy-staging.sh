#!/usr/bin/env bash
# infrastructure/scripts/deploy-staging.sh
# ─────────────────────────────────────────────────────────────────────────────
# Pre-staging 部署脚本（消费 release/staging-<commit>/ 目录）。
#
# 由 GitLab CI 的 deploy:pre-staging 任务通过 SSH 调用；也可由 ops 直接跑。
#
# 工作流：
#   1. 校验 $STAGING_COMMIT env var（CI 必传；手动跑也必传）
#   2. preflight：docker / compose 可用、infrastructure/ 目录齐
#   3. docker compose pull 拉 commit-pinned 镜像
#   4. 按依赖顺序逐服务 `up -d --no-deps`，每容器等 healthcheck 通过
#   5. frontend:8080/health 公共入口门禁
#   6. image prune
#
# 与 deploy-remote.sh 的区别：
#   - 不需要 git checkout：整个 pre-staging 目录自包含（脚本随包走）
#   - compose 已 transformed：image: 引用 <registry>/<repo>/<svc>:<commit>
#     不再做本地 build，每次部署都是 commit-pinned 的固定版本
#   - 拉镜像走 registry（不 docker save）
#
# 退出码：
#   0   全部服务健康通过
#   10  preflight fail
#   20+ pull fail
#   30+ 滚动重启中某服务 health timeout
#   40  frontend /health 公共入口不通
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ─── STAGING_COMMIT 必传 ────────────────────────────────────────────────────────
# CI: deploy:pre-staging 任务 STAGING_COMMIT=$CI_COMMIT_SHORT_SHA bash ...
# 手动: STAGING_COMMIT=<sha> bash scripts/deploy-staging.sh
if [[ -z "${STAGING_COMMIT:-}" ]]; then
  echo "[deploy-staging] FATAL: STAGING_COMMIT env var is required" >&2
  echo "  CI:    STAGING_COMMIT=\$CI_COMMIT_SHORT_SHA bash $0" >&2
  echo "  手动: cd <staging-dir>/ && STAGING_COMMIT=<sha> bash scripts/deploy-staging.sh" >&2
  exit 10
fi

log()  { echo "[deploy-staging commit=${STAGING_COMMIT}] $*"; }
die()  { echo "[deploy-staging commit=${STAGING_COMMIT}] FATAL: $*" >&2; exit "${2:-10}"; }

# ─── preflight ────────────────────────────────────────────────────────────────
log "STEP 0/5  preflight"

command -v docker >/dev/null 2>&1 || die "docker not in PATH" 10
docker compose version >/dev/null 2>&1 || die "docker compose plugin missing (need v2.20+ for !override tag)" 10

COMPOSE_BASE="${PROJECT_ROOT}/infrastructure/docker-compose.yml"
COMPOSE_PROD="${PROJECT_ROOT}/infrastructure/docker-compose.prod.yml"
DOCKER_COMPOSE=(docker compose -f "${COMPOSE_BASE}")
DEPLOY_MODE="dev"
if [[ -f "${COMPOSE_PROD}" ]]; then
  DOCKER_COMPOSE+=(-f "${COMPOSE_PROD}")
  DEPLOY_MODE="prod"
fi

for f in "${COMPOSE_BASE}" "${PROJECT_ROOT}/infrastructure/docker-compose-services.yml"; do
  [[ -f "${f}" ]] || die "missing compose file: ${f}" 10
done

# dev compose（pre-staging 用 prod 模式：prod overlay 存在时走 prod；否则 dev）。
# 不叠加 docker-compose-infra.dev.yml（dev 端口暴露仅本地开发用，staging 走默认网络）。
case "${DEPLOY_MODE}" in
  prod)
    log "  compose files: ${DOCKER_COMPOSE[*]}"
    log "  ⚠ prod mode: required env vars will be validated by upstream preflight (deploy-remote.sh logic skipped here)"
    # prod 模式不强制校验 .env.prod（pre-staging 不一定有 .env.prod）。
    # 若上游 CI 没注入关键 var，容器启动失败即可由 healthcheck 捕获。
    ;;
  dev)
    log "  compose files: ${DOCKER_COMPOSE[*]}"
    ;;
esac

# 待重启的服务列表（顺序 = 依赖顺序：基础设施优先，业务服务最后）。
# 与 deploy-remote.sh 保持一致；outbox-publisher 类无 healthcheck 用启动窗口判断。
SERVICES=(config-service file-service \
           token-service iam-service iam-outbox-publisher \
           notification-service notification-outbox-publisher \
           audit-service prue_backend \
           api-gateway frontend)
log "  services (in order): ${SERVICES[*]}"

# ─── pull images ──────────────────────────────────────────────────────────────
log "STEP 1/5  docker compose pull (commit=${STAGING_COMMIT})"
if ! "${DOCKER_COMPOSE[@]}" pull; then
  die "docker compose pull failed — check registry reachability / image:${STAGING_COMMIT} tag exists" 20
fi

# ─── rolling restart ─────────────────────────────────────────────────────────
log "STEP 2/5  rolling restart (infra untouched)"

HEALTH_TIMEOUT_SECS="${HEALTH_TIMEOUT_SECS:-60}"
HEALTH_POLL_INTERVAL_SECS="${HEALTH_POLL_INTERVAL_SECS:-5}"
HEALTH_MAX_POLLS=$((HEALTH_TIMEOUT_SECS / HEALTH_POLL_INTERVAL_SECS))

for svc in "${SERVICES[@]}"; do
  log "  → rolling ${svc}"
  if ! "${DOCKER_COMPOSE[@]}" up -d --no-deps "${svc}"; then
    die "compose up failed for ${svc}" 30
  fi

  ok=0
  for i in $(seq 1 "${HEALTH_MAX_POLLS}"); do
    sleep "${HEALTH_POLL_INTERVAL_SECS}"
    # docker compose v2 默认容器名是 `<project>-<service>-<replica>`，不是 `${svc}` 本身。
    # 直接 `docker inspect "${svc}"` 永远找不到容器（staging 实际容器名形如
    # `infrastructure-config-service-1`），导致 health 一直是空串、12 轮全超时。
    # 用 `docker compose ps -q` 拿容器 ID，对 project 命名约定不敏感。
    container_id="$("${DOCKER_COMPOSE[@]}" ps -q "${svc}" 2>/dev/null | head -n1 || true)"
    if [[ -z "${container_id}" ]]; then
      health="starting"  # 容器还没起：继续轮询
    else
      health="$(docker inspect --format='{{.State.Health.Status}}' "${container_id}" 2>/dev/null || echo starting)"
    fi
    log "    health[$i/${HEALTH_MAX_POLLS}]: ${health}"
    case "${health}" in
      healthy)
        ok=1; break ;;
      none)
        # outbox-publisher 类无 HEALTHCHECK 指令：给启动窗口 10s 后视为 ok
        sleep 10
        ok=1; break ;;
      starting)
        : # 继续轮询
        ;;
      *)
        # unhealthy / unhealthy-ish：让循环继续跑，可能自愈；最后 ok=0 时报错
        ;;
    esac
  done

  if [[ "${ok}" -eq 0 ]]; then
    log "  ✘ ${svc} 未通过健康检查（超时 ${HEALTH_TIMEOUT_SECS}s），最近 60 条日志：" >&2
    # 同样用容器 ID 拉日志（service 名不会在 docker 里找得到）
    container_id="$("${DOCKER_COMPOSE[@]}" ps -q "${svc}" 2>/dev/null | head -n1 || true)"
    if [[ -n "${container_id}" ]]; then
      docker logs --tail=60 "${container_id}" >&2 || true
    else
      echo "    (容器 ${svc} 不存在 — compose up -d 可能失败)" >&2
    fi
    die "service ${svc} healthcheck timeout" 31
  fi
done

# ─── entrypoint verification ─────────────────────────────────────────────────
log "STEP 3/5  entrypoint verification"
HEALTH_URL="${HEALTH_URL:-http://localhost:8080/health}"
HEALTH_OK=0
for i in $(seq 1 12); do
  sleep 5
  if curl -fsS --max-time 3 "${HEALTH_URL}" >/dev/null 2>&1; then
    log "    ✓ ${HEALTH_URL} 返回 2xx (attempt $i/12)"
    HEALTH_OK=1; break
  fi
  log "    · ${HEALTH_URL} 未通 (attempt $i/12)"
done
if [[ "${HEALTH_OK}" -eq 0 ]]; then
  die "frontend health probe failed at ${HEALTH_URL}" 40
fi

# ─── image prune ──────────────────────────────────────────────────────────────
log "STEP 4/5  docker image prune -f"
docker image prune -f || true

# ─── 总结 ────────────────────────────────────────────────────────────────────
log "STEP 5/5  done"
log "  commit: ${STAGING_COMMIT}"
log "  frontend health: ${HEALTH_URL}"
log "DONE — pre-staging 已就绪（commit=${STAGING_COMMIT}）"