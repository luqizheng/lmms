#!/bin/sh
# 双角色镜像的角色感知健康探针。
#
# 背景：iam-service / notification-service 的镜像里同时装着**两个**二进制——
# 服务端（iam_service / notification_service，监听 HTTP 端口）和 outbox publisher
# （iam_outbox_publisher / ntf_outbox_publisher，无监听端口的 worker），
# compose 用 `command:` 切换角色。但 Docker 的 HEALTHCHECK 是**镜像级**的，
# 写死了服务端端口 → publisher 角色永远探不到端口，容器一直显示 unhealthy。
#
# 这里按 PID 1 的 cmdline 判角色：
#   服务端角色   → 探 HTTP /health（真实就绪信号）
#   publisher   → 无监听端口，改查 Prometheus 指标文件的新鲜度：publisher 每轮
#                 轮询会原子写一次该文件（METRICS_FILE_PATH），mtime 新鲜即
#                 轮询循环还活着；卡死则 mtime 停更。
#
# 用法：Dockerfile 里 HEALTHCHECK CMD 调用本脚本，服务端角色需给
# HEALTHCHECK_PORT 环境变量（各自的服务端口）。
set -eu

cmdline="$(tr '\000' ' ' < /proc/1/cmdline 2>/dev/null || true)"

case "$cmdline" in
  *outbox_publisher*)
    metrics="${METRICS_FILE_PATH:-}"
    if [ -z "$metrics" ]; then
      # 各 publisher 的默认输出路径不同，按二进制名兜底
      case "$cmdline" in
        *iam_outbox_publisher*) metrics=/var/run/prometheus/iam_outbox.prom ;;
        *ntf_outbox_publisher*) metrics=/var/run/prometheus/ntf_outbox.prom ;;
        *) exit 1 ;;
      esac
    fi
    [ -f "$metrics" ] || exit 1
    now="$(date +%s)"
    mtime="$(stat -c %Y "$metrics" 2>/dev/null || echo 0)"
    # 轮询间隔默认 1s（OUTBOX_POLL_INTERVAL_MS），留 60s 余量；超时即视为卡死
    [ "$((now - mtime))" -le 60 ] || exit 1
    exit 0
    ;;
  *)
    port="${HEALTHCHECK_PORT:-}"
    [ -n "$port" ] || exit 1
    wget --spider -q "http://127.0.0.1:${port}/health" || exit 1
    ;;
esac
