# run-tests-docker 共享 MySQL 容器复用

日期：2026-07-06
范围：`scripts/run-tests-docker.js`

## 背景

`startSharedMysql()` 每次都 `docker rm -f coder-test-mysql` + 全新 `docker run`，
即使上一轮的容器还活着、健康可用。warm run 测得：

- cold run：45s（compile 21s + mysql 10s + test 5s + overhead）
- warm run：20s（compile <1s + **mysql 10s** + test 5s + overhead）

warm 时 `target/` 增量已极致，MySQL 容器冷启是剩余最大固定开销。

## 目标

- 已存在且 `mysqladmin ping` 通过的 `coder-test-mysql` 直接复用，跳过 ~10s 容器冷启
- 端口处理：固定 `33306`（替换当前 `33000 + random*1000`）

## 非目标

- 不解决并发跑多次 `run-tests-docker.js` 抢同一个容器的竞争（pre-existing）
- 不持久化 `/var/lib/mysql`（shared 模式下 test 自己 `CREATE DATABASE`，挂 volume 收益≈0）
- 不改 `ensureNextest()` 的 probe docker run（另一个独立小优化）

## 端口处理：三种方案

| 方案 | 描述 | 优 | 劣 |
|---|---|---|---|
| **A 固定端口 + 复用（推荐）** | `MYSQL_PORT = 33306` 常量；inspect 存在则复用，否则 rm + run with fixed port | 代码最简；端口可预测便于 debug；33306 是高位端口冲突概率极低 | 多实例并发同脚本会互抢（pre-existing）|
| B 随机端口 + label 持久化 | `docker run --label mysql-port=33456`；inspect 读 label | 端口保持随机，多实例互不踩 | 要管 label 生命周期；inspect --format 取 label 字符串解析；复杂度 > 收益 |
| C 容器名带 PID 后缀 | `coder-test-mysql-${pid}`，从不复用 | 并发安全 | 直接抵消本次优化目的 |

**选 A**：`run-tests-docker.js` 是单机 dev/CI 串行使用，并发不是 use case；
复用 + 固定端口两行解决，比 B 少一个 label 维护点。

## 改动

- `scripts/run-tests-docker.js:39` 新增 `const MYSQL_PORT = 33306;`
- `scripts/run-tests-docker.js:118` `startSharedMysql()` 头部加 inspect + ping 短路：
  - `docker inspect --type=container coder-test-mysql` → 容器存在
  - `docker exec coder-test-mysql mysqladmin ping -uroot --port=33306` → `mysqld is alive`
  - 两者都通过 → `return { port: 33306 }`，跳过 rm + run
- 不存在或不健康 → 走原 rm + run 流程（端口换成 33306）
- `scripts/run-tests-docker.js:164` `stopSharedMysql()` 改为 no-op：保留容器供下次 run 复用。
  - 想强制重建：`docker rm -f coder-test-mysql` 一条命令搞定

## 验证

- 跑 `node scripts/run-tests-docker.js notification-service` 多次
- warm run 实测目标 <5s（当前 ~20s，省下 ~15-16s）
- 失败兜底：inspect 不存在 → 走原路径，不影响 cold run

## 影响面

- 共享 MySQL 容器名、行为不变，CI 不感知
- 已存在的 `coder-test-mysql`（旧随机端口）首次启动会被新逻辑 rm 重建一次，之后稳定