# 44-fu2 · actor_id 在 iam.permission.changed 事件中可追溯

Phase 4.4-fu2 端到端验证：admin 改权限 → audit_events.actor_id 非空。

## 步骤

1. admin 拿 admin_token；建 user `u_target` + role `r1`
2. admin `PUT /iam/roles/r1/permissions` 绑定某 permission → 触发 `iam.permission.changed`
3. `mysql> SELECT JSON_EXTRACT(envelope_json, '$.payload.actor_id') FROM iam_outbox_events WHERE event_type='iam.permission.changed' ORDER BY id DESC LIMIT 1;`
   预期: 返回 `"<admin_user_id>"` 而非 `null`
4. 进一步：`mysql> SELECT actor_id FROM audit_events WHERE event_type='iam.permission.changed' ORDER BY id DESC LIMIT 1;`
   预期: actor_id 列非空
