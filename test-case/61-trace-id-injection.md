# T1: envelope.trace_id producer 自动注入 — 手工验收

## 前置

```bash
cd /home/kudi/projects/templates/low-memory-micro-services
docker compose -f infrastructure/docker-compose-infra.yml up -d
docker compose -f infrastructure/docker-compose-services.yml up -d
```

如启用 observability：再加 `infrastructure/docker-compose-observability.yml`。

## 步骤

### 1. 单元测试（无 Docker）

```bash
cargo test -p common-telemetry --test current_trace_id_test
cargo test -p common-event --test enqueue_trace_id_test
```

期望：两组都全绿。`enqueue_trace_id_test` 覆盖 `fill_trace_id` 的三分支：
- `trace_id=None` + source 返 Some → 注入
- `trace_id=Some` + source 返 Some → 保留 caller
- `trace_id=None` + source 返 None → 保持 None

### 2. 集成测试（需 Docker）

```bash
cargo test -p common-event --test enqueue_writes_trace_id_to_db
```

期望：1 passed。验证 caller 传入的 `trace_id` 真的写入 MySQL `iam_outbox_events.envelope_json`。

如本机无 Docker：CI 环境跑，本机跳过即可。

### 3. iam-service 集成测试回归

```bash
cargo test -p iam-service
```

期望：全绿。Phase 4.4 六个 case + Phase 4 I5 / I2 等历史 case 不应因 `&mut Envelope` 改动回归。

### 4. 端到端 traceparent 透传

```bash
# 起全栈后
node scripts/e2e-trace-id-flow.js
```

脚本行为：
1. 登录 admin
2. 携带 `traceparent: 00-aabbccddeeff00112233445566778899-1122334455667788-01` 调用 `POST /v1/iam/users`
3. 轮询 audit_events，直到对应 `user_id` 行的 `trace_id` 落库
4. 断言 `audit trace_id == aabbccddeeff00112233445566778899`

期望：脚本退出码 0，stdout 最后一行 `PASS — audit trace_id=... matches caller's W3C trace_id`。

### 5. 反例（无 traceparent）

手测：去掉 traceparent header，正常创建用户：

```bash
curl -X POST http://localhost:3000/v1/iam/users \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"username":"no_trace_test","password":"P@ssw0rd!","nickname":"no-trace"}'

sleep 5

docker exec infrastructure-mysql-1 mysql -uroot -prootpass -N -B -e "
SELECT IFNULL(trace_id,'')
FROM audit_events
WHERE event_type='iam.user.created'
  AND JSON_UNQUOTE(JSON_EXTRACT(payload_json,'\$.user_id'))='${USER_ID}'
ORDER BY id DESC LIMIT 1;" superpowers
```

期望：`trace_id` 字段非空、为 32 位 hex（fallback），但**与 traceparent 透传用例的 trace_id 不相同**（说明这是 fallback 而不是透传）。

> 注：fallback 32-hex 由 `gateway trace_request_middleware` 在无上游 traceparent 时生成，存到当前 span 的 OTel context 中；common-telemetry 的 `current_trace_id()` 从中读取。
> 若 iam 未接 OTel collector，fallback 路径不会触发，`trace_id` 留空（fill_trace_id source 返回 None）。

## 验收标准

- [ ] 单元测 `current_trace_id_test` 全绿
- [ ] 单元测 `enqueue_trace_id_test` 全绿（3 case）
- [ ] 集成测 `enqueue_writes_trace_id_to_db` 1 passed
- [ ] iam-service 集成测试无回归
- [ ] `scripts/e2e-trace-id-flow.js` 退出码 0
- [ ] 无 traceparent 时 trace_id 非空且与透传用例不同