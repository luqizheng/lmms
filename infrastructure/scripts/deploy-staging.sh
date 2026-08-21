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

# ─── env-file：compose 插值用，缺失会导致 ${DATABASE_URL} 等落成空串，
#     应用 panic "DATABASE_URL is set but empty"（pipeline 6533 / job 23772）。
#     PROJECT_ROOT/.env（随包）+ STAGING_ENV_FILE（CI/ops 覆盖，后者优先）。
ENV_FILES=()
if [[ -f "${PROJECT_ROOT}/.env" ]]; then
  ENV_FILES+=(--env-file "${PROJECT_ROOT}/.env")
  log "  env-file: ${PROJECT_ROOT}/.env (bundled)"
fi
if [[ -n "${STAGING_ENV_FILE:-}" ]]; then
  if [[ -f "${STAGING_ENV_FILE}" ]]; then
    ENV_FILES+=(--env-file "${STAGING_ENV_FILE}")
    log "  env-file: ${STAGING_ENV_FILE} (STAGING_ENV_FILE)"
  else
    die "STAGING_ENV_FILE set but not found: ${STAGING_ENV_FILE}" 10
  fi
fi
if [[ ${#ENV_FILES[@]} -eq 0 ]]; then
  log "  ⚠ no --env-file; relying on exported shell env"
fi
DOCKER_COMPOSE+=("${ENV_FILES[@]}")

# Fail-fast：缺关键 env 直接退出，比 60s 健康检查超时好定位。
REQUIRED_VARS=(DATABASE_URL JWKS_URL)
if [[ "${DEPLOY_MODE}" == "prod" ]]; then
  REQUIRED_VARS+=(MYSQL_HOST MYSQL_USER MYSQL_PASSWORD MYSQL_DATABASE)
fi
for ef in "${ENV_FILES[@]}"; do
  [[ "$ef" == "--env-file" ]] && continue
  while IFS= read -r line || [[ -n "$line" ]]; do
    [[ -z "$line" || "$line" =~ ^[[:space:]]*# || "$line" != *=* ]] && continue
    key="${line%%=*}"
    val="${line#*=}"
    val="${val%\"}"; val="${val#\"}"; val="${val%\'}"; val="${val#\'}"
    if [[ -z "${!key:-}" ]]; then
      printf -v "$key" '%s' "$val"
    fi
  done < "$ef"
done
for v in "${REQUIRED_VARS[@]}"; do
  if [[ -z "${!v:-}" ]]; then
    die "required env ${v} empty — set it in STAGING_ENV_FILE or PROJECT_ROOT/.env (template: infrastructure/.env.prod.example)" 10
  fi
done
log "  env check: ${REQUIRED_VARS[*]} all set"

# dev compose（pre-staging 用 prod 模式：prod overlay 存在时走 prod；否则 dev）。
# 不叠加 docker-compose-infra.dev.yml（dev 端口暴露仅本地开发用，staging 走默认网络）。
case "${DEPLOY_MODE}" in
  prod)
    log "  compose files: ${DOCKER_COMPOSE[*]}"
    ;;
  dev)
    log "  compose files: ${DOCKER_COMPOSE[*]}"
    ;;
esac

# 待重启的服务列表（顺序 = 依赖顺序：基础设施优先，业务服务最后）。
# 与 deploy-remote.sh 保持一致。
SERVICES=(config-service file-service \
           token-service iam-service iam-outbox-publisher \
           notification-service notification-outbox-publisher \
           audit-service prue_backend \
           api-gateway frontend)
log "  services (in order): ${SERVICES[*]}"

# 后台 worker 服务：无 HTTP 端口，不复用 HTTP 服务的 HEALTHCHECK 判定。
# outbox-publisher 复用 iam-service/notification-service 的 Dockerfile（带针对
# /health 的 HEALTHCHECK），但跑的是不同二进制、不监听端口，继承的 HEALTHCHECK 必然
# 失败 → 容器永远 unhealthy，导致部署超时。对这类服务只验证容器处于 running 状态，
# 给一个启动窗口即视为就绪。
is_worker_service() {
  case "$1" in
    iam-outbox-publisher|notification-outbox-publisher) return 0 ;;
    *) return 1 ;;
  esac
}

# ─── pull images ──────────────────────────────────────────────────────────────
log "STEP 1/5  docker compose pull (commit=${STAGING_COMMIT})"
if ! "${DOCKER_COMPOSE[@]}" pull; then
  die "docker compose pull failed — check registry reachability / image:${STAGING_COMMIT} tag exists" 20
fi

# ─── infra warm-up ──────────────────────────────────────────────────────────
# Fresh pre-staging/<sha> dirs have no running containers. Rolling restart
# below uses `--no-deps` (so it never touches infra), which means the business
# services would otherwise come up before mysql/redis/consul/rabbitmq and
# crash-loop with "Name does not resolve" (job 24677: config-service DNS
# failure on `mysql`). Bring infra up first and block on mysql healthcheck —
# this is idempotent on an existing deployment (already-healthy containers
# are no-ops).
log "STEP 2/6  ensuring infrastructure services (mysql/redis/consul/rabbitmq)"
if ! "${DOCKER_COMPOSE[@]}" up -d mysql redis consul rabbitmq; then
  die "compose up failed for infrastructure services" 25
fi

# Wait for mysql healthy (config-service / iam-service depend on it via
# depends_on: condition: service_healthy; --no-deps below bypasses that gate).
INFRA_TIMEOUT_SECS="${INFRA_TIMEOUT_SECS:-120}"
INFRA_POLL_INTERVAL_SECS="${INFRA_POLL_INTERVAL_SECS:-5}"
INFRA_MAX_POLLS=$((INFRA_TIMEOUT_SECS / INFRA_POLL_INTERVAL_SECS))
mysql_ok=0
for i in $(seq 1 "${INFRA_MAX_POLLS}"); do
  sleep "${INFRA_POLL_INTERVAL_SECS}"
  mysql_id="$("${DOCKER_COMPOSE[@]}" ps -q mysql 2>/dev/null | head -n1 || true)"
  if [[ -z "${mysql_id}" ]]; then
    log "    mysql[$i/${INFRA_MAX_POLLS}]: container not found yet"
    continue
  fi
  mysql_health="$(docker inspect --format='{{.State.Health.Status}}' "${mysql_id}" 2>/dev/null || echo starting)"
  log "    mysql[$i/${INFRA_MAX_POLLS}]: ${mysql_health}"
  if [[ "${mysql_health}" == "healthy" ]]; then
    mysql_ok=1; break
  fi
done
if [[ "${mysql_ok}" -eq 0 ]]; then
  log "  ✘ mysql 未在 ${INFRA_TIMEOUT_SECS}s 内就绪，最近 40 条日志：" >&2
  mysql_id="$("${DOCKER_COMPOSE[@]}" ps -q mysql 2>/dev/null | head -n1 || true)"
  [[ -n "${mysql_id}" ]] && docker logs --tail=40 "${mysql_id}" >&2 || true
  die "mysql infrastructure not healthy within ${INFRA_TIMEOUT_SECS}s" 26
fi

# ─── rolling restart ─────────────────────────────────────────────────────────
log "STEP 3/6  rolling restart (business services)"

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

    # 后台 worker 无 HTTP 端口：不依赖继承的 HEALTHCHECK（必然 unhealthy），
    # 只确认容器进入 running 状态即视为就绪。
    if is_worker_service "${svc}"; then
      state="$(docker inspect --format='{{.State.Running}}' "${container_id}" 2>/dev/null || echo false)"
      log "    worker[$i/${HEALTH_MAX_POLLS}]: running=${state} (health=${health})"
      if [[ "${state}" == "true" ]]; then
        ok=1; break
      fi
      continue
    fi

    log "    health[$i/${HEALTH_MAX_POLLS}]: ${health}"
    case "${health}" in
      healthy)
        ok=1; break ;;
      none)
        # 无 HEALTHCHECK 指令：给启动窗口 10s 后视为 ok
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
log "STEP 4/6  entrypoint verification"
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
log "STEP 5/6  docker image prune -f"
docker image prune -f || true

# ─── 总结 ────────────────────────────────────────────────────────────────────
log "STEP 6/6  done"
log "  commit: ${STAGING_COMMIT}"
log "  frontend health: ${HEALTH_URL}"
log "DONE — pre-staging 已就绪（commit=${STAGING_COMMIT}）"