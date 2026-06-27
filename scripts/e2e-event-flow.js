#!/usr/bin/env node
// Phase 4 event-flow smoke: create a user via api-gateway → assert the
// iam.user.created event lands in audit_events via the iam-outbox-publisher
// → RabbitMQ events exchange → audit.iam-events queue → audit-service consumer.
//
// Prereqs:
//   - infrastructure/docker-compose-infra.yml infra is up (mysql + rabbitmq + consul + redis)
//   - infrastructure/docker-compose-services.yml services are up (api-gateway + iam-service + iam-outbox-publisher + audit-service)
//   - admin user "e2eadmin" / "P@ssw0rd!" exists (globalSetup behavior of tests/e2e)
//
// Usage:
//   node scripts/e2e-event-flow.js
//   (exits 0 on success, 1 on failure)

const { execSync } = require('node:child_process');

const GATEWAY = process.env.E2E_BASE_URL || 'http://localhost:3000';
const ADMIN = process.env.E2E_ADMIN_USER || 'e2eadmin';
const PASS = process.env.E2E_ADMIN_PASS || 'P@ssw0rd!';
const MYSQL_CONTAINER = process.env.E2E_MYSQL_CONTAINER || 'infrastructure-mysql-1';
const TIMEOUT_MS = 30_000;
const POLL_MS = 500;

async function httpJson(method, path, body, token) {
  const url = GATEWAY + path;
  const headers = { 'Content-Type': 'application/json' };
  if (token) headers.Authorization = `Bearer ${token}`;
  const res = await fetch(url, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  let json;
  try { json = text ? JSON.parse(text) : {}; } catch { json = { _raw: text }; }
  return { status: res.status, body: json };
}

function mysqlScalar(sql) {
  const cmd = `docker exec ${MYSQL_CONTAINER} mysql -uroot -prootpass -N -B -e ${JSON.stringify(sql)} superpowers`;
  return execSync(cmd, { encoding: 'utf-8', stdio: ['pipe', 'pipe', 'ignore'] }).trim();
}

async function main() {
  console.log('[e2e-event-flow] login as admin');
  const login = await httpJson('POST', '/v1/iam/auth/login', { username: ADMIN, password: PASS });
  if (login.status !== 200 || !login.body.access_token) {
    console.error(`login failed: ${login.status}`, login.body);
    process.exit(1);
  }
  const token = login.body.access_token;

  const username = `evt_${Date.now()}_${Math.floor(Math.random() * 1e6)}`;
  console.log(`[e2e-event-flow] create user ${username}`);
  const create = await httpJson('POST', '/v1/iam/users', {
    username,
    password: 'Evt1234!Evt1234!',
    nickname: 'event-flow-test',
  }, token);
  if (create.status !== 200 && create.status !== 201) {
    console.error(`create_user failed: ${create.status}`, create.body);
    process.exit(1);
  }
  const userId = create.body.id;
  console.log(`[e2e-event-flow] user created id=${userId}`);

  console.log('[e2e-event-flow] polling iam_outbox_events for the row...');
  let outboxId = '';
  const deadline = Date.now() + TIMEOUT_MS;
  while (Date.now() < deadline) {
    try {
      outboxId = mysqlScalar(
        `SELECT id FROM iam_outbox_events WHERE event_type='iam.user.created' AND JSON_UNQUOTE(JSON_EXTRACT(envelope_json,'$.payload.user_id'))='${userId}' LIMIT 1`
      );
      if (outboxId) break;
    } catch (e) { /* mysql transient — retry */ }
    await new Promise((r) => setTimeout(r, POLL_MS));
  }
  if (!outboxId) {
    console.error('FAIL: outbox row never appeared');
    process.exit(1);
  }
  console.log(`[e2e-event-flow] outbox row id=${outboxId}`);

  console.log('[e2e-event-flow] polling for published_at + audit_events row...');
  let publishedAt = '';
  let auditEventId = '';
  while (Date.now() < deadline) {
    try {
      publishedAt = mysqlScalar(
        `SELECT IFNULL(published_at,'') FROM iam_outbox_events WHERE id=${outboxId}`
      );
      auditEventId = mysqlScalar(
        `SELECT event_id FROM audit_events WHERE event_type='iam.user.created' AND JSON_UNQUOTE(JSON_EXTRACT(payload_json,'$.user_id'))='${userId}' LIMIT 1`
      );
      if (publishedAt && auditEventId) break;
    } catch (e) { /* retry */ }
    await new Promise((r) => setTimeout(r, POLL_MS));
  }
  if (!publishedAt) {
    console.error('FAIL: iam_outbox_events.published_at never set (publisher not running?)');
    process.exit(1);
  }
  if (!auditEventId) {
    console.error('FAIL: audit_events row never appeared (consumer not running?)');
    process.exit(1);
  }
  console.log(`[e2e-event-flow] PASS — published_at=${publishedAt} audit event_id=${auditEventId}`);
  process.exit(0);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
