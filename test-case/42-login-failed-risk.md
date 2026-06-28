# 42 — 登录失败事件 + 风控（Phase 4.2）

**链路**：iam 四种登录失败路径（bad_password / user_disabled / password_expired /
user_not_found）→ 同事务写 `iam_login_logs` + 发 `iam.login.failed`（outbox）→ RabbitMQ。
两个消费者：① audit 经 `iam.#` 落 `audit_events`（审计）；② audit `audit.login-risk` 队列做
**风控滑动窗口计数**（Redis），同一 username 或 IP 在窗口内失败次数 == 阈值（默认 5/300s）时
打 WARN 日志 + 递增 Prometheus `login_risk_alerts_total`。

前置：`00-setup.md` 完成。

## 场景 42-A：单次登录失败发出事件 + 审计落库

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| A1 | 用错误密码登录一次：<br>`curl -s -X POST $BASE/v1/iam/auth/login -H 'Content-Type: application/json' -d '{"username":"admin","password":"WRONG"}'` | 401/400 拒绝 | | |
| A2 | 查 iam 失败日志：<br>`mysqlq "SELECT username,success,error_message,created_at FROM iam_login_logs WHERE success=0 ORDER BY created_at DESC LIMIT 3"` | 有一行 `success=0`，`error_message=bad_password` | | |
| A3 | 查 outbox：<br>`mysqlq "SELECT event_type,JSON_EXTRACT(envelope_json,'$.payload.reason') reason FROM iam_outbox_events WHERE event_type='iam.login.failed' ORDER BY id DESC LIMIT 3"` | 有 `iam.login.failed`，reason=`bad_password` | | |
| A4 | 等 1~3s 查审计落库：<br>`mysqlq "SELECT event_type,occurred_at FROM audit_events WHERE event_type='iam.login.failed' ORDER BY occurred_at DESC LIMIT 3"` | audit_events 出现该事件（`iam.#` 自动落库） | | |

## 场景 42-B：未知用户也发事件（user_not_found，user_id=NULL）

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| B1 | 登录一个不存在的用户：<br>`curl -s -X POST $BASE/v1/iam/auth/login -H 'Content-Type: application/json' -d '{"username":"ghost_user_xyz","password":"x"}'` | 401/400 | | |
| B2 | `mysqlq "SELECT username,user_id,error_message FROM iam_login_logs WHERE username='ghost_user_xyz'"` | 一行，`user_id` 为 **NULL**（不是空串），`error_message=user_not_found` | | |
| B3 | `mysqlq "SELECT JSON_EXTRACT(envelope_json,'$.payload.reason'),JSON_EXTRACT(envelope_json,'$.payload.user_ids') FROM iam_outbox_events WHERE event_type='iam.login.failed' ORDER BY id DESC LIMIT 1"` | reason=`user_not_found` | | |

## 场景 42-C：风控滑动窗口越线告警（核心）

清干净计数，连续失败 5 次，验证恰好第 5 次告警。

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| C1 | 清掉该 username 的风控计数：<br>`redisq DEL "audit:loginfail:user:risktarget"` | OK（或 0） | | |
| C2 | 先确保存在用户 `risktarget`（创建一个，或对已存在用户用错密码）。用错密码连打 **5** 次：<br>`for i in 1 2 3 4 5; do curl -s -o /dev/null -X POST $BASE/v1/iam/auth/login -H 'Content-Type: application/json' -d '{"username":"risktarget","password":"WRONG"}'; done` | 5 次都 401 | | |
| C3 | 看 Redis 计数：<br>`redisq GET "audit:loginfail:user:risktarget"` | `5`（带 TTL：`redisq TTL audit:loginfail:user:risktarget` > 0 且 ≤ 300） | | |
| C4 | 看 audit 日志是否出现越线 WARN：<br>`docker compose -f infrastructure/docker-compose-services.yml logs audit-service \| grep -i "threshold exceeded"` | 出现一条 `login failure threshold exceeded`（dimension=user） | | |
| C5 | 看 Prometheus 指标：<br>`curl -s localhost:6005/metrics \| grep login_risk_alerts_total` | `login_risk_alerts_total{dimension="user"} 1`（或 ≥1） | | |
| C6 | 第 6 次再失败：`curl ... password WRONG` 一次 | 不再新增 WARN（同窗口只告警一次）；计数变 6 | | |

## 场景 42-D：Redis 抖动不卡消息（可选，⚠️ 破坏性）

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| D1 | ⚠️ 停 Redis：`docker stop infrastructure-redis-1` | | | |
| D2 | 失败登录一次 | 仍 401；audit 不崩、消息仍被 ack（看日志有 `risk counter redis error` warn，无堆积） | | |
| D3 | RabbitMQ 管理台看 `audit.login-risk` | 无消息持续堆积（handler 吞错 ack） | | |
| D4 | 恢复：`docker start infrastructure-redis-1` | | | |

## 验收判定

- ✅ 通过：四类失败都发事件 + 落 iam_login_logs；user_not_found 的 user_id 为 NULL；
  审计自动落 audit_events；风控第 5 次恰好告警一次 + 指标 +1；Redis 抖动不卡队列。
- ❌ 失败常见原因：outbox 没发（publisher）；audit 未订阅 `audit.login-risk`（看日志
  `risk consumer subscribed`）；阈值/窗口环境变量被改（`LOGIN_RISK_THRESHOLD` / `LOGIN_RISK_WINDOW_SECS`）。
