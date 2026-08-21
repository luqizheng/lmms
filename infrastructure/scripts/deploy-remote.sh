#!/usr/bin/env bash
# infrastructure/scripts/deploy-remote.sh
# ─────────────────────────────────────────────────────────────────────────────
# Remote deployment orchestrator. 由 .gitlab-ci.yml 的 deploy job 通过 SSH
# 调用；也可直接由 ops `./deploy-remote.sh` 触发同等流程。
#
# 行为：
#   1. preflight 检查（docker compose version + 关键 env vars + 是否有 prod overlay）
#   2. pull images
#   3. 按依赖顺序逐服务 `up -d --no-deps`，每个容器等 healthcheck（或启动窗口）通过
#   4. 清理 dangling images
#
# 退出码：
#   0   全部服务健康通过
#   10  preflight fail（compose 版本 / 关键 vars 缺失）
#   20+ pull fail
#   30+ 滚动重启中某服务 health timeout
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${PROJECT_ROOT:-$(cd "$SCRIPT_DIR/../.." && pwd)}"

_prepend_docker_paths() {
  local _d _found=""
  for _d in /usr/bin /usr/local/bin /snap/bin /opt/docker/bin; do
    [ -x "$_d/docker" ] && _found="$_d:$_found"
  done
  for _d in /usr/libexec/docker/cli-plugins \
            /usr/local/libexec/docker/cli-plugins \
            "$HOME/.docker/cli-plugins"; do
    [ -x "$_d/docker-compose" ] && _found="$_d:$_found"
  done
  [ -n "$_found" ] && export PATH="${_found}${PATH}"
}
_prepend_docker_paths

_sudo_apt_install() {
  # 先探 sudo -n 是否可用，避免把无密码提示的失败当成 install 成功吞掉。
  if ! command -v sudo >/dev/null 2>&1; then
    echo "[deploy-remote] no sudo binary on PATH — cannot self-install" >&2
    return 1
  fi
  if ! sudo -n true 2>/dev/null; then
    echo "[deploy-remote] sudo -n denied (NOPASSWD not configured); cannot self-install" >&2
    return 1
  fi
  sudo -n apt-get update -qq && sudo -n apt-get install -y "$@"
}

if ! command -v docker >/dev/null 2>&1; then
  echo "[deploy-remote] docker CLI not found; attempting apt install docker.io"
  _sudo_apt_install docker.io || true
fi

if ! command -v docker-compose >/dev/null 2>&1 && ! docker compose version >/dev/null 2>&1; then
  echo "[deploy-remote] docker compose plugin not found; attempting apt install"
  _sudo_apt_install docker-compose-plugin || true
  _prepend_docker_paths
fi

if ! command -v docker >/dev/null 2>&1; then
  {
    echo "[deploy-remote] FATAL: docker CLI not available on PATH"
    echo "  --- diagnostic dump ---"
    printf '  whoami     : %s\n' "$(whoami 2>/dev/null || echo unknown)"
    printf '  groups     : %s\n' "$(groups 2>/dev/null | tr ' ' ',' || echo unknown)"
    printf '  PATH       : %s\n' "$PATH"
    printf '  docker     : %s\n' "$(command -v docker 2>/dev/null || echo '<NOT FOUND>')"
    printf '  compose-v2 : %s\n' "$(docker compose version 2>/dev/null || echo '<NOT FOUND>')"
    printf '  candidates :\n'
    for _c in /usr/bin/docker /usr/local/bin/docker /snap/bin/docker \
              /usr/libexec/docker/cli-plugins/docker-compose \
              /usr/local/libexec/docker/cli-plugins/docker-compose \
              "$HOME/.docker/cli-plugins/docker-compose"; do
      if [ -e "$_c" ]; then
        printf '    %s (mode %s owner %s)\n' "$_c" \
          "$(stat -c '%a' "$_c" 2>/dev/null || echo '?')" \
          "$(stat -c '%U:%G' "$_c" 2>/dev/null || echo '?')"
      else
        printf '    %s (missing)\n' "$_c"
      fi
    done
    echo "  --- end diagnostic ---"
    echo "  hint: 在 staging 上以 kudi 身份登录后跑"
    echo "    command -v docker; type docker; echo \"\$PATH\""
    echo "  并比对 SSH login-shell 与 non-login-shell 的 PATH。"
  } >&2
  exit 10
fi

cd "$PROJECT_ROOT" || { echo "[deploy-remote] FATAL: cannot cd to $PROJECT_ROOT" >&2; exit 10; }

# ─────────────────────────────────────────────────────────────────────────────
# Compose 文件集：base + 可选 prod overlay
# ─────────────────────────────────────────────────────────────────────────────
COMPOSE_BASE="$PROJECT_ROOT/infrastructure/docker-compose.yml"
COMPOSE_PROD="$PROJECT_ROOT/infrastructure/docker-compose.prod.yml"
DOCKER_COMPOSE=(docker compose -f "$COMPOSE_BASE")
if [[ -f "$COMPOSE_PROD" ]]; then
  DOCKER_COMPOSE+=(-f "$COMPOSE_PROD")
  DEPLOY_MODE="prod"
else
  DEPLOY_MODE="dev"
fi

log() { echo "[deploy-remote $DEPLOY_MODE] $*"; }
die() { echo "[deploy-remote $DEPLOY_MODE] FATAL: $*" >&2; exit "${2:-10}"; }

# ─────────────────────────────────────────────────────────────────────────────
# STEP 0 — preflight
# ─────────────────────────────────────────────────────────────────────────────
log "STEP 0/4  preflight checks"

# 0a. compose 版本
if ! docker compose version >/dev/null 2>&1; then
  die "docker compose not available in PATH" 10
fi
COMPOSE_VER="$(docker compose version --short 2>/dev/null || docker compose version | head -1)"
log "  docker compose version: $COMPOSE_VER"

if [[ "$DEPLOY_MODE" == "prod" ]]; then
  # 0b. !override 标签需要 v2.20+
  if ! docker compose version | grep -qE 'v?2\.(2[0-9]|[3-9][0-9])'; then
    die "docker compose v2.20+ required for !override tag (found: $COMPOSE_VER)" 10
  fi

  # 0c. 关键 .env.prod 变量必须存在且非默认值
  REQUIRED=(
    MYSQL_ROOT_PASSWORD MYSQL_USER MYSQL_PASSWORD DATABASE_URL
    RABBITMQ_DEFAULT_PASS
    IAM_DEV_BT TOKEN_DEV_BT NOTIFICATION_DEV_BT AUDIT_DEV_BT
    PRUE_DEV_BT GATEWAY_DEV_BT CONFIG_DEV_BT FILE_DEV_BT
  )
  for v in "${REQUIRED[@]}"; do
    if [[ -z "${!v:-}" ]]; then
      die "required env var $v is empty (set in .env.prod)" 10
    fi
    if [[ "${!v}" == "__SET_BY_OPS__" ]]; then
      die "env var $v is still placeholder __SET_BY_OPS__ (fill .env.prod)" 10
    fi
    if [[ "${!v}" == dev-bt-* ]]; then
      die "env var $v looks like a dev fallback value (fill .env.prod)" 10
    fi
  done

  # 0d. compose config 必须能成功渲染（!override 标签崩溃会让 compose config 报错）
  log "  docker compose config (validate rendered tree)"
  if ! "${DOCKER_COMPOSE[@]}" config --quiet >/dev/null 2>&1; then
    die "docker compose config failed — check docker-compose.prod.yml syntax / !override" 10
  fi
fi

# 0e. 待重启的服务列表（顺序 = 依赖顺序：基础设施依赖优先，业务服务最后）
SERVICES=(config-service file-service \
           token-service iam-service iam-outbox-publisher \
           notification-service notification-outbox-publisher \
           audit-service prue_backend \
           api-gateway frontend)

# 后台 worker 服务：无 HTTP 端口，不复用 HTTP 服务的 HEALTHCHECK 判定。
# outbox-publisher 复用 iam/notification 的 Dockerfile（带 /health HEALTHCHECK），
# 但跑不同二进制、不监听端口，继承的 HEALTHCHECK 必然失败 → 永远 unhealthy 致部署超时。
is_worker_service() {
  case "$1" in
    iam-outbox-publisher|notification-outbox-publisher) return 0 ;;
    *) return 1 ;;
  esac
}

log "  compose files: ${DOCKER_COMPOSE[*]}"
log "  services (in order): ${SERVICES[*]}"

# ─────────────────────────────────────────────────────────────────────────────
# STEP 1 — pull images
# ─────────────────────────────────────────────────────────────────────────────
log "STEP 1/4  docker compose pull"
if ! "${DOCKER_COMPOSE[@]}" pull; then
  die "docker compose pull failed" 20
fi

# ─────────────────────────────────────────────────────────────────────────────
# STEP 1.5 — infra warm-up (mysql / redis / consul / rabbitmq)
#   Fresh installs or hosts where `docker compose down` was run will have no
#   infra containers. The rolling restart below uses --no-deps, so without
#   this step business services crash-loop on "Name does not resolve" for
#   `mysql` (see deploy-staging job 24677). Idempotent on healthy hosts.
# ─────────────────────────────────────────────────────────────────────────────
log "STEP 2/5  ensuring infrastructure services (mysql/redis/consul/rabbitmq)"
if ! "${DOCKER_COMPOSE[@]}" up -d mysql redis consul rabbitmq; then
  die "compose up failed for infrastructure services" 25
fi

INFRA_TIMEOUT_SECS="${INFRA_TIMEOUT_SECS:-120}"
INFRA_POLL_INTERVAL_SECS="${INFRA_POLL_INTERVAL_SECS:-5}"
INFRA_MAX_POLLS=$((INFRA_TIMEOUT_SECS / INFRA_POLL_INTERVAL_SECS))
mysql_ok=0
for i in $(seq 1 "$INFRA_MAX_POLLS"); do
  sleep "$INFRA_POLL_INTERVAL_SECS"
  mysql_id="$("${DOCKER_COMPOSE[@]}" ps -q mysql 2>/dev/null | head -n1 || true)"
  if [[ -z "${mysql_id}" ]]; then
    log "    mysql[$i/$INFRA_MAX_POLLS]: container not found yet"
    continue
  fi
  mysql_health="$(docker inspect --format='{{.State.Health.Status}}' "${mysql_id}" 2>/dev/null || echo starting)"
  log "    mysql[$i/$INFRA_MAX_POLLS]: $mysql_health"
  if [[ "$mysql_health" == "healthy" ]]; then
    mysql_ok=1; break
  fi
done
if [[ "$mysql_ok" -eq 0 ]]; then
  log "  ✘ mysql 未在 ${INFRA_TIMEOUT_SECS}s 内就绪，最近 40 条日志：" >&2
  mysql_id="$("${DOCKER_COMPOSE[@]}" ps -q mysql 2>/dev/null | head -n1 || true)"
  [[ -n "$mysql_id" ]] && docker logs --tail=40 "$mysql_id" >&2 || true
  die "mysql infrastructure not healthy within ${INFRA_TIMEOUT_SECS}s" 26
fi

# ─────────────────────────────────────────────────────────────────────────────
# STEP 2 — rolling restart by service
# ─────────────────────────────────────────────────────────────────────────────
log "STEP 3/5  rolling restart (business services)"

HEALTH_TIMEOUT_SECS="${HEALTH_TIMEOUT_SECS:-60}"
HEALTH_POLL_INTERVAL_SECS="${HEALTH_POLL_INTERVAL_SECS:-5}"
HEALTH_MAX_POLLS=$((HEALTH_TIMEOUT_SECS / HEALTH_POLL_INTERVAL_SECS))

for svc in "${SERVICES[@]}"; do
  log "  → rolling $svc"
  if ! "${DOCKER_COMPOSE[@]}" up -d --no-deps "$svc"; then
    die "compose up failed for $svc" 30
  fi

  ok=0
  for i in $(seq 1 "$HEALTH_MAX_POLLS"); do
    sleep "$HEALTH_POLL_INTERVAL_SECS"
    # docker compose v2 默认容器名是 `<project>-<service>-<replica>`，不是 `$svc` 本身。
    # 直接 `docker inspect "$svc"` 永远找不到容器（实际容器名形如
    # `infrastructure-config-service-1`），导致 health 一直是空串、12 轮全超时。
    # 用 `docker compose ps -q` 拿容器 ID，对 project 命名约定不敏感。
    container_id="$("${DOCKER_COMPOSE[@]}" ps -q "$svc" 2>/dev/null | head -n1 || true)"
    if [[ -z "$container_id" ]]; then
      health="starting"  # 容器还没起：继续轮询
    else
      health="$(docker inspect --format='{{.State.Health.Status}}' "$container_id" 2>/dev/null || echo starting)"
    fi

    # 后台 worker 无 HTTP 端口：不依赖继承的 HEALTHCHECK（必然 unhealthy），
    # 只确认容器进入 running 状态即视为就绪。
    if is_worker_service "$svc"; then
      state="$(docker inspect --format='{{.State.Running}}' "$container_id" 2>/dev/null || echo false)"
      log "    worker[$i/$HEALTH_MAX_POLLS]: running=$state (health=$health)"
      if [[ "$state" == "true" ]]; then
        ok=1; break
      fi
      continue
    fi

    log "    health[$i/$HEALTH_MAX_POLLS]: $health"
    case "$health" in
      healthy)
        ok=1; break ;;
      none|starting)
        # 无 HEALTHCHECK 指令：给启动窗口 10s 后视为 ok
        if [[ "$health" == "none" ]]; then
          sleep 10
          ok=1; break
        fi
        ;;
    esac
  done

  if [[ "$ok" -eq 0 ]]; then
    log "  ✘ $svc 未通过健康检查（超时 ${HEALTH_TIMEOUT_SECS}s），最近 60 条日志：" >&2
    # 同样用容器 ID 拉日志（service 名不会在 docker 里找得到）
    container_id="$("${DOCKER_COMPOSE[@]}" ps -q "$svc" 2>/dev/null | head -n1 || true)"
    if [[ -n "$container_id" ]]; then
      docker logs --tail=60 "$container_id" >&2 || true
    else
      echo "    (容器 $svc 不存在 — compose up -d 可能失败)" >&2
    fi
    die "service $svc healthcheck timeout" 31
  fi
done

# ─────────────────────────────────────────────────────────────────────────────
# STEP 4 — entrypoint verification（验证公共入口可访问）
# ─────────────────────────────────────────────────────────────────────────────
log "STEP 4/5  entrypoint verification"
# frontend:8080 是公共入口；其容器内 nginx 用 `location /health` 返回 200
HEALTH_URL="${HEALTH_URL:-http://localhost:8080/health}"
HEALTH_OK=0
for i in $(seq 1 12); do
  sleep 5
  if curl -fsS --max-time 3 "$HEALTH_URL" >/dev/null 2>&1; then
    log "    ✓ $HEALTH_URL returned 2xx (attempt $i/12)"
    HEALTH_OK=1; break
  fi
  log "    · $HEALTH_URL not yet (attempt $i/12)"
done
if [[ "$HEALTH_OK" -eq 0 ]]; then
  die "frontend health probe failed at $HEALTH_URL" 32
fi

# ─────────────────────────────────────────────────────────────────────────────
# STEP 5 — image prune
# ─────────────────────────────────────────────────────────────────────────────
log "STEP 5/5  docker image prune -f"
docker image prune -f || true

log "DONE — all services healthy on port 8080"
