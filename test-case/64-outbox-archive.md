# T4: Outbox 24h Archive — 手工 e2e 验收

## 前置

```bash
cd /home/kudi/projects/templates/low-memory-micro-services
docker compose -f infrastructure/docker-compose-infra.yml up -d
docker compose -f infrastructure/docker-compose-services.yml up -d
```

> 关注 `iam-outbox-publisher` 起来；启动日志里有
> `outbox archive cron started`（interval=5min, retention=24h, purge=30d）

## 步骤

### 1. 集成测试（需 Docker,本地或 CI 跑）

```bash
cargo test -p iam-service --test outbox_archive_test -- --test-threads=1
```

期望：7 个用例全绿（`archive_moves_published_rows_older_than_24h`、
`archive_skips_unpublished_rows`、`archive_batches_at_1000`、
`archive_get_lock_prevents_concurrent_runs`、
`purge_deletes_old_archive_rows`、
`archive_window_picks_up_late_published`、
`publisher_main_loop_unaffected_during_archive`）。

### 2. 构造数据

创建 5 个用户 → 5 条 `iam_outbox_events`
等 5 秒（publisher 发完，`published_at` 全填）

### 3. 人工调老 published_at

```bash
mysql -h 127.0.0.1 -u root -prootpass superpowers -e "
UPDATE iam_outbox_events
SET published_at = NOW(3) - INTERVAL 25 HOUR
WHERE event_type = 'iam.user.created';
"
```

（注：5 分钟 tick 才能反映生产 cron 节奏；可改代码 `ARCHIVE_TICK=5s` 加快）

### 4. 等 cron tick

```bash
sleep 30
```

### 5. 验证

```bash
mysql -h 127.0.0.1 -u root -prootpass superpowers -e "
SELECT COUNT(*) AS live_count FROM iam_outbox_events;
SELECT COUNT(*) AS archive_count FROM iam_outbox_events_archive;
"
```

期望：`live_count = 0`（如果本 batch 是 5 个 user），`archive_count = 5`

### 6. archive 表 30 天 purge 验证

```bash
mysql -h 127.0.0.1 -u root -prootpass superpowers -e "
INSERT INTO iam_outbox_events_archive
  (id, event_id, event_type, routing_key, envelope_json, created_at, published_at, archived_at)
VALUES
  (999999, UUID(), 'ev.purge.test', 'test', '{}', NOW(3), NOW(3), NOW(3) - INTERVAL 31 DAY);
"
sleep 30
mysql -h 127.0.0.1 -u root -prootpass superpowers -e "
SELECT COUNT(*) FROM iam_outbox_events_archive WHERE event_type = 'ev.purge.test';
"
```

期望：0（purged）

### 7. 多副本互斥验证

```bash
# 起两个 publisher 实例
cargo run --bin outbox-publisher &
cargo run --bin outbox-publisher &
# 两个进程都应启动，archive cron 只在一个里跑。
# 看日志：另一个应该看到 debug 级别 "another replica holds archive lock, skipping"
RUST_LOG=debug cargo run --bin outbox-publisher 2>&1 | grep "archive lock"
```

期望：只有一个进程每 5 分钟执行归档工作；另一个持续 `skipping`。

## 验收标准

- [ ] archive cron 启动日志可见
- [ ] 24h+ 行自动迁移到 archive 表
- [ ] unpublished 行不被误伤
- [ ] archive 表 30 天行被 purge
- [ ] publisher 主循环不阻塞（archive 跑时新事件仍能正常 publish）
- [ ] 多副本时 archive 互斥（仅一个进程跑归档）
