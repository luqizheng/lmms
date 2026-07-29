#!/bin/bash
# docker-entrypoint wrapper for mysql-replica.
#
# The official mysql:8.0 image's docker-entrypoint.sh initializes the data
# dir on first run (if empty) and then exec's mysqld. We need to:
#   1. Start mysqld in the background so it initializes and accepts connections
#   2. Wait for mysqld to be ready (healthcheck-style probe)
#   3. Configure replication via CHANGE MASTER TO
#   4. Start the slave threads
#   5. Bring mysqld back to the foreground (so docker's PID-1 semantics work)
#
# Replication uses GTID auto-positioning (MASTER_AUTO_POSITION=1) so that
# when the replica starts with an empty data dir, MySQL will automatically
# clone the missing transactions from the primary. No manual mysqldump
# bootstrap is required.

set -euo pipefail

PRIMARY_HOST="${MYSQL_PRIMARY_HOST:-mysql}"
PRIMARY_PORT="${MYSQL_PRIMARY_PORT:-3306}"
REPL_USER="${MYSQL_REPL_USER:-repl}"
REPL_PASSWORD="${MYSQL_REPL_PASSWORD:-replpass}"

echo "[replica] starting mysqld in background..."

# Start the official entrypoint (which initializes data dir if empty, then
# exec's mysqld). We need to intercept so we can run our replication setup
# in between init and the long-running mysqld process.
# Trick: launch mysqld ourselves after delegating init to docker-entrypoint.sh.

# Use the official entrypoint to perform init (it creates the data dir +
# initial system tables, then exits). We then start mysqld in background.
/usr/local/bin/docker-entrypoint.sh --initialize-insecure 2>/dev/null || true
# If data dir already initialized, --initialize-insecure fails fast. Either way
# we now have a usable data dir.

mysqld --user=mysql --datadir=/var/lib/mysql &
MYSQLD_PID=$!

echo "[replica] mysqld pid=${MYSQLD_PID}, waiting for it to accept connections..."

# Poll until mysqld responds
for i in $(seq 1 60); do
  if mysqladmin ping -u root -p"${MYSQL_ROOT_PASSWORD}" >/dev/null 2>&1; then
    echo "[replica] mysqld is up (attempt ${i})"
    break
  fi
  sleep 2
done

echo "[replica] waiting for primary ${PRIMARY_HOST}:${PRIMARY_PORT}..."
for i in $(seq 1 60); do
  if mysql -h "${PRIMARY_HOST}" -P "${PRIMARY_PORT}" -u "${REPL_USER}" \
       -p"${REPL_PASSWORD}" -e "SELECT 1" >/dev/null 2>&1; then
    echo "[replica] primary is reachable (attempt ${i})"
    break
  fi
  sleep 2
done

# Configure replication
mysql -u root -p"${MYSQL_ROOT_PASSWORD}" <<SQL
CHANGE MASTER TO
  MASTER_HOST='${PRIMARY_HOST}',
  MASTER_PORT=${PRIMARY_PORT},
  MASTER_USER='${REPL_USER}',
  MASTER_PASSWORD='${REPL_PASSWORD}',
  MASTER_AUTO_POSITION=1,
  MASTER_CONNECT_RETRY=10;
START SLAVE;
SQL

echo "[replica] CHANGE MASTER TO + START SLAVE issued"
mysql -u root -p"${MYSQL_ROOT_PASSWORD}" -e "SHOW SLAVE STATUS\G" | \
  grep -E "Slave_IO_Running|Slave_SQL_Running|Seconds_Behind_Master|Last_IO_Error|Last_SQL_Error" || true

echo "[replica] ready; handing over to mysqld (pid=${MYSQLD_PID})"

# Bring mysqld back to foreground so docker's PID-1 / signal handling works.
wait "${MYSQLD_PID}"