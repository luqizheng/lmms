# MySQL 主从复制管理 Runbook

> **状态**：初版 · 可执行
> **适用范围**：coder-micro-services 平台 MySQL 主从架构
> **前置条件**：`docker compose -f infrastructure/docker-compose.yml up -d` 已成功启动，`mysql` 容器 healthcheck 通过
> **词法约定**：以下 `<compose>` 表示 `docker compose -f infrastructure/docker-compose.yml`

---

## 0. 一句话

- **GTID-based async replication**，1 主 1 从（默认），从库只读
- **冷启动**：从库空 data dir + `CHANGE MASTER TO MASTER_AUTO_POSITION=1` —— MySQL 自动从主库克隆缺失事务，无需手动 mysqldump bootstrap
- **灾备**：主库挂掉 → promote 从库为主 → 修改服务连接串 → 重启业务服务
- **读写分离**：post-1.0，本 runbook 不覆盖

---

## 1. 一次性配置（主库首次启用时）

主库已通过 `docker-compose-infra.yml` 的 `command` 自动开启 `--gtid-mode=ON` 和 `--log-bin=mysql-bin`，但**复制用户需要手工创建一次**（`infrastructure/mysql-init/` 由 docker daemon 创建后权限为 root，无法直接写入 init SQL）：

```bash
<compose> exec mysql mysql -u root -p"${MYSQL_ROOT_PASSWORD}" <<SQL
CREATE USER IF NOT EXISTS '${MYSQL_REPL_USER}'@'%'
  IDENTIFIED WITH mysql_native_password BY '${MYSQL_REPL_PASSWORD}';
GRANT REPLICATION SLAVE, REPLICATION CLIENT ON *.* TO '${MYSQL_REPL_USER}'@'%';
FLUSH PRIVILEGES;
SQL
```

**验证主库复制位点**：

```bash
<compose> exec mysql mysql -u root -p"${MYSQL_ROOT_PASSWORD}" -e "
  SHOW MASTER STATUS\G
  SELECT @@server_uuid, @@gtid_executed;
"
```

预期输出：
- `File: mysql-bin.000001`
- `Position: 某个非零数字`
- `Executed_Gtid_Set: 非空`（应用过至少一次事务后才有值）

---

## 2. 启动从库

```bash
# 启动（会自动 CHANGE MASTER TO + START SLAVE）
<compose> up -d mysql-replica

# 查看日志（首次启动可能需要 30-60s 等待主库可达）
<compose> logs -f mysql-replica

# 验证从库状态
<compose> exec mysql-replica mysql -u root -p"${MYSQL_ROOT_PASSWORD}" -e "
  SHOW SLAVE STATUS\G
" | grep -E "Slave_IO_Running|Slave_SQL_Running|Seconds_Behind_Master|Master_UUID"
```

**健康标志**：
- `Slave_IO_Running: Yes`
- `Slave_SQL_Running: Yes`
- `Seconds_Behind_Master: 0`（理想值；少量秒数可接受）
- `Master_UUID: <主库 server_uuid>`

---

## 3. 日常监控

### 3.1 延迟检查

```bash
<compose> exec mysql-replica mysql -u root -p"${MYSQL_ROOT_PASSWORD}" -e "
  SHOW SLAVE STATUS\G
" | grep -E "Seconds_Behind_Master"
```

阈值建议：
- `< 5s`：正常
- `5s - 60s`：观察，通知 oncall
- `> 60s`：阻塞告警，需介入

### 3.2 同步失败告警

已有 Prometheus 告警规则 `OutboxBacklog` 涵盖部分场景，但 MySQL 复制延迟不在监控范围。**短期方案**：在 cron 加以下巡检：

```bash
#!/bin/bash
# check-mysql-replication.sh
DELAY=$(<compose> exec -T mysql-replica \
  mysql -u root -p"${MYSQL_ROOT_PASSWORD}" -N -B \
  -e "SHOW SLAVE STATUS\G" | grep Seconds_Behind_Master | awk '{print $2}')

if [ "$DELAY" -gt 60 ]; then
  echo "ALERT: MySQL replica lag ${DELAY}s exceeds 60s threshold"
  # send alert (Slack / PagerDuty / etc.)
fi
```

> **Roadmap**：2.0 将 `Seconds_Behind_Master` 加入 Prometheus 抓取（`mysql_slave_status` exporter）。

---

## 4. 常见故障处理

### 4.1 从库断开重连

```bash
<compose> exec mysql-replica mysql -u root -p"${MYSQL_ROOT_PASSWORD}" -e "
  STOP SLAVE;
  START SLAVE;
  SHOW SLAVE STATUS\G
"
```

### 4.2 复制错误（Last_SQL_Error）

```bash
<compose> exec mysql-replica mysql -u root -p"${MYSQL_ROOT_PASSWORD}" -e "
  STOP SLAVE;
  SET GLOBAL sql_slave_skip_counter = 1;  -- 跳过 1 条错误事务
  START SLAVE;
"
```

**慎用**：仅当确认错误事务可被安全跳过（开发人员的 DDL 错误、误操作等）时使用。生产 DDL 错误应通过完整主从重建解决。

### 4.3 从库 data dir 损坏

```bash
# 1. 停从库
<compose> stop mysql-replica

# 2. 删除损坏的 data dir（docker volume）
docker volume rm coder-micro-services_mysql_replica_data

# 3. 重启（会自动重新拉取所有事务）
<compose> up -d mysql-replica
```

---

## 5. 主从切换（Failover）

### 5.1 主库宕机 → 提升从库

```bash
# 1. 停止从库复制
<compose> exec mysql-replica mysql -u root -p"${MYSQL_ROOT_PASSWORD}" -e "
  STOP SLAVE;
  RESET SLAVE ALL;
"

# 2. 解除从库只读（开放写入）
<compose> exec mysql-replica mysql -u root -p"${MYSQL_ROOT_PASSWORD}" -e "
  SET GLOBAL read_only = OFF;
"

# 3. 验证可写
<compose> exec mysql-replica mysql -u root -p"${MYSQL_ROOT_PASSWORD}" -e "
  CREATE DATABASE _failover_test;
  DROP DATABASE _failover_test;
"
```

### 5.2 业务服务切到从库

业务服务的 `DATABASE_URL` 通过环境变量配置（`infrastructure/.env`）：

```bash
# 1. 改 .env 中的 MYSQL_HOST
sed -i 's/^MYSQL_HOST=.*/MYSQL_HOST=mysql-replica/' infrastructure/.env

# 2. 同步所有业务服务的 DATABASE_URL（继承 ${MYSQL_HOST}）
#    如果各服务有自己的 DATABASE_URL，需要逐个修改

# 3. 滚动重启所有业务服务（CI 中已有健康检查门禁逻辑）
<compose> up -d \
  iam-service token-service audit-service config-service \
  file-service notification-service prue_backend api-gateway
```

### 5.3 旧主库恢复（重建从库）

旧主库恢复后**不能再当主库**——它的 binlog 落后太多。改为新主库（mysql-replica）的从库：

```bash
# 旧主库上：擦除旧数据，启动为新从库
<compose> stop mysql
docker volume rm coder-micro-services_mysql_data
<compose> up -d mysql  # 启动后为空 data dir，自动重建

# 新主库（mysql-replica）上：创建 replication user（如果还没创建）
# （与 §1 相同）

# 旧主库作为新从库：
#   改 .env：MYSQL_HOST=mysql-replica, MYSQL_PRIMARY_HOST=mysql-replica
#   启动 mysql 服务时会自动 CHANGE MASTER TO
```

> **简化方案（推荐）**：failover 后直接销毁旧主库容器，**不再恢复为从库**。下次需要时用 §2 的流程从零启动一个干净的从库。代价是需要从主库重新克隆数据（~10 分钟取决于数据量），收益是流程简单、不出错。

---

## 6. 何时升级为主从切换演练

- 首次部署后：演练一次（操作员熟悉流程）
- 季度：完整 failover 演练 + 业务验证
- 重大变更前：升级前演练

演练建议在 `STAGING` 环境执行，避免生产事故。

---

## 7. 与 backup.md 的关系

| 场景 | 用 backup.md | 用本 runbook |
|------|-------------|-------------|
| 主库数据损坏，需从 .sql 恢复 | ✅ | — |
| 主库硬件故障，需快速恢复服务 | — | ✅ Failover |
| 主库 + 从库都挂（数据中心级灾难） | ✅ | 之后 ✅ |
| 日常巡检延迟 | — | ✅ §3 |
| 从库因 binlog 漂移需重建 | — | ✅ §4.3 |

---

## 8. 已知限制

- **同步延迟**：async replication 有秒级延迟，**写入后立即读从库**可能读到旧数据。需要 read-your-writes 语义的业务（用户登录后立刻查用户列表）应继续读主库。
- **failover 数据丢失**：async replication 在主库崩溃时可能丢失最后几秒事务。真正不能丢数据的场景需要 semi-sync replication（2.0 路线）。
- **无自动故障转移**：本 runbook 是手工 SOP。如需自动 failover，引入 MHA / Orchestrator（2.0 评估）。
- **主从切换有秒级服务中断**：业务服务需要重新建连，DNS 缓存可能导致更长的中断。2.0 引入 Consul-aware 连接池可缩短中断。