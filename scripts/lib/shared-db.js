// 共享测试数据库（MySQL / Redis）探活与拉起。
//
// 容器由 scripts/run-tests-docker.js 或人工创建（coder-test-mysql / coder-test-redis）。
// 主机侧脚本只做「探活 + 启动已有容器」，不创建、不删除——两个脚本共用同一套容器，
// 谁创建都行，但都不要拆对方的台。

const net = require('node:net');
const { spawnSync } = require('node:child_process');

const MYSQL_CONTAINER = 'coder-test-mysql';
const REDIS_CONTAINER = 'coder-test-redis';
const PG_CONTAINER = 'coder-test-pg';

/** TCP 端口探活（只判断"有没有人在听"，不校验协议）。 */
function probeTcp(host, port, timeoutMs = 1500) {
  return new Promise((resolve) => {
    const socket = new net.Socket();
    let settled = false;
    const done = (ok) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      resolve(ok);
    };
    socket.setTimeout(timeoutMs);
    socket.once('connect', () => done(true));
    socket.once('timeout', () => done(false));
    socket.once('error', () => done(false));
    socket.connect(port, host);
  });
}

/** 容器存在但没跑时把它拉起来。返回 true 表示拉起过。 */
function startExistingContainer(name) {
  const inspect = spawnSync('docker', ['inspect', '--type=container', name], { encoding: 'utf8' });
  if (inspect.status !== 0) return false;
  const start = spawnSync('docker', ['start', name], { stdio: 'inherit' });
  return start.status === 0;
}

/**
 * 确保 MySQL 可用：先探活，不通就尝试拉起已有容器，再探一次。
 * @returns {Promise<boolean>} 是否可用
 */
async function ensureMysql(host, port, log = console.log) {
  if (await probeTcp(host, port)) return true;
  log(`[db] mysql ${host}:${port} 不可达，尝试拉起容器 ${MYSQL_CONTAINER}`);
  if (startExistingContainer(MYSQL_CONTAINER)) {
    // 容器起来后 mysqld 还要几秒才监听
    for (let i = 0; i < 30; i++) {
      await new Promise((r) => setTimeout(r, 1000));
      if (await probeTcp(host, port)) return true;
    }
  }
  return false;
}

/**
 * 确保 Redis 可用。
 * @returns {Promise<boolean>} 是否可用
 */
async function ensureRedis(host, port, log = console.log) {
  if (await probeTcp(host, port)) return true;
  log(`[db] redis ${host}:${port} 不可达，尝试拉起容器 ${REDIS_CONTAINER}`);
  if (startExistingContainer(REDIS_CONTAINER)) {
    for (let i = 0; i < 30; i++) {
      await new Promise((r) => setTimeout(r, 1000));
      if (await probeTcp(host, port)) return true;
    }
  }
  return false;
}

/**
 * 确保 PostgreSQL 可用（coder-test-pg，postgres:16）。
 * 与 ensureMysql 同款「探活 + 拉起已有容器」协议。
 * @returns {Promise<boolean>} 是否可用
 */
async function ensurePostgres(host, port, log = console.log) {
  if (await probeTcp(host, port)) return true;
  log(`[db] postgres ${host}:${port} 不可达，尝试拉起容器 ${PG_CONTAINER}`);
  if (startExistingContainer(PG_CONTAINER)) {
    for (let i = 0; i < 30; i++) {
      await new Promise((r) => setTimeout(r, 1000));
      if (await probeTcp(host, port)) return true;
    }
  }
  return false;
}

module.exports = {
  MYSQL_CONTAINER,
  REDIS_CONTAINER,
  PG_CONTAINER,
  probeTcp,
  startExistingContainer,
  ensureMysql,
  ensureRedis,
  ensurePostgres,
};
