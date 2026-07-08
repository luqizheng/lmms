# 5b-fu3 · notification membership 投影端到端验证

Phase 5B-fu3 端到端：notification 启动一次性 bootstrap → 持续消费
`notification.membership-events`（3 个 routing-key）→ `publish_announcement`
走本地 `ntf_user_projection` 解析 role/dept scope（无同步 HTTP）。

## 前置

- 启动完整 dev compose（`pnpm run dev:infra` + `pnpm run dev`），含 iam/notification/rabbitmq
- rabbitmq management UI 已就绪（`http://localhost:15672`）

## 步骤

### 1. 准备 role + dept + user

```bash
# 创建 role r1
POST /iam/roles { code: "r1_test", name: "R1 Test" }
# 记录 role_id (假设 R1_ID)
# 创建 dept d1
POST /iam/org-units { code: "d1_test", name: "D1 Test" }
# 记录 dept_id (假设 D1_ID)
# 创建 user u1 (含 role r1 + dept d1)
POST /iam/users { username: "u1", password: "p@ssw0rd!", nickname: "U1", role_ids: [R1_ID] }
# 记录 user_id (假设 U1_ID)
# 把 u1 加到 d1
PUT /iam/org-units/D1_ID/users { user_ids: [U1_ID] }
```

### 2. 验证 consumer 收到事件 + 投影表更新

```bash
# 拉一次 notification 的 metric, 看 consumer 状态
curl -s notification:8080/metrics | grep membership
# 预期: ntf_membership_consumer_running 1 (启动 OK)
```

```sql
-- 投影表应该有 u1, role_ids=["<R1_ID>"], dept_id="<D1_ID>", last_event_at>0
SELECT user_id, role_ids, dept_id, last_event_at
FROM ntf_user_projection WHERE user_id = '<U1_ID>';
-- 预期: 1 行, role_ids JSON 数组含 R1_ID, dept_id=D1_ID, last_event_at > 0
```

### 3. 验证 bootstrap 重启后正确还原

```bash
# 删投影行模拟"投影丢失"
mysql -e "DELETE FROM ntf_user_projection WHERE user_id = '<U1_ID>';"
# 重启 notification
docker restart notification-service
# 等待 5s
sleep 5
# bootstrap 应重建该行
mysql -e "SELECT user_id, JSON_LENGTH(role_ids), dept_id FROM ntf_user_projection WHERE user_id = '<U1_ID>';"
# 预期: role_ids 长度 >= 1, dept_id=<D1_ID>
```

### 4. 验证 publish 走本地投影（无 HTTP）

```bash
# 创建 announcement 受众 = [role: R1_ID]
POST /notification/announcements { title: "t", body: "b", publish_at: "<now>", recipients: [{scope_type: "role", scope_id: "<R1_ID>"}] }
# 记录 ann_id
# publish
POST /notification/announcements/<ANN_ID>/publish
# 预期: 返回 200, user_count >= 1
```

```sql
-- deliveries 应有 U1_ID (projection 命中)
SELECT user_id FROM ntf_announcement_deliveries WHERE announcement_id = <ANN_ID>;
-- 预期: 包含 U1_ID
```

```bash
# 检查 notification 日志: 不应有 iam/internal/users/by-role HTTP 调用
docker logs notification-service 2>&1 | grep -E "iam/internal/users/by-role" | head
# 预期: 0 行
```

### 5. 验证 out-of-order 兜底

```bash
# 重复 assign 同一个 role (同 user_id, 更小 ts) — 用低层 SQL 模拟事件乱序
mysql -e "INSERT INTO iam_outbox_events (event_id, event_type, routing_key, envelope_json, created_at) \
  VALUES (UUID(), 'iam.user.role.assigned', 'iam.user.role.assigned', \
  JSON_OBJECT('event_id', UUID(), 'event_type', 'iam.user.role.assigned', \
              'schema_version', 1, 'producer', 'iam-service', 'occurred_at', DATE_FORMAT(NOW(), '%Y-%m-%dT%H:%i:%SZ'), \
              'tenant_id', '1', 'payload', JSON_OBJECT('user_id', '<U1_ID>', 'role_id', 'r1_test', 'tenant_id', 1, 'actor_id', NULL, 'ts', 1)), \
  NOW());"
# 等待 3s
sleep 3
# projection 的 last_event_at 不应被覆盖 (因为 ts=1 < 现有 last_event_at)
mysql -e "SELECT last_event_at FROM ntf_user_projection WHERE user_id = '<U1_ID>';"
# 预期: last_event_at > 1 (即保持原值)
```

### 6. 验证 role 禁用 → unassigned 事件 → 投影移除

```bash
# 把 r1 禁用 (会触发 role_disabled + role_unassigned × 1)
PUT /iam/roles/<R1_ID> { status: "disabled" }
# 等待 3s
sleep 3
mysql -e "SELECT role_ids FROM ntf_user_projection WHERE user_id = '<U1_ID>';"
# 预期: JSON_ARRAY (空数组, role 已被移除)
```

```bash
# 重新启用 r1, 重新 assign 给 u1
PUT /iam/roles/<R1_ID> { status: "active" }
POST /iam/users/<U1_ID>/roles { role_ids: ["<R1_ID>"] }
sleep 3
mysql -e "SELECT role_ids FROM ntf_user_projection WHERE user_id = '<U1_ID>';"
# 预期: JSON_ARRAY 含 R1_ID
```

## 失败信号

- 步骤 2 查不到 projection 行 → consumer 未启动或事件未发出
- 步骤 4 publish 报 5xx → projection 为空或 SQL 出错
- 步骤 4 log 中出现 `iam/internal/users/by-role` → HTTP 路径未完全替换（应已废弃）
- 步骤 6 last_event_at 被覆盖 → out-of-order 兜底逻辑缺失

## 关联

- Spec: `docs/superpowers/specs/2026-07-08-phase5b-fu3-notification-iam-event-sync-design.md`
- Plan: `docs/superpowers/plans/2026-07-08-phase5b-fu3-notification-iam-event-sync.md`