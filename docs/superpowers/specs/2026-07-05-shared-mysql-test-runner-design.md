# 测试编排：共享 MySQL + 多随机 database — 设计

日期：2026-07-05
状态：执行中

## 目标

`scripts/run-tests-docker.js` 跑全量时，每个 service 在自己的测试 setup 里通过 `testcontainers_modules::mysql::Mysql::default().start()` 各起一个 MySQL 容器。8 个 service 串行 + 并发 2 时，启动 ~8 个 MySQL 容器，每次冷启 5–10s、内存 200–400MB × 8。

改造方向：**一个共享 MySQL 容器，每个 service 进程独立随机 database**。

## 设计决策摘要

| 决策 | 选择 | 理由 |
|------|------|------|
| 容器数 | 单 MySQL 容器，所有 service 共享 | 收益最大；复杂度最低 |
| 数据库隔离 | 每 service 进程独立 `test_<svc>_<pid>_<rand>` database | 真隔离，不依赖表前缀约定；database name 含 pid+rand 防并发冲突 |
| 共享触发 | 环境变量 `TEST_MYSQL_HOST` / `TEST_MYSQL_PORT` 存在即走共享模式 | 零代码改动即可开关；本地 `cargo test`（不经脚本）走 testcontainers 分支保留 |
| 容器端口 | 随机 33000–33999 高位端口 + `--network host` | 与 service container 共享 host 网络栈，免端口映射；避免与本机 mysql 3306 冲突 |
| MySQL 镜像 | `mysql:8.0` | 现有 service 测试都跑 8.0；`--innodb-use-native-aio=0` 保留（绕宿主 aio-max-nr EAGAIN） |
| MySQL `max_connections` | `--max-connections=500` | 8 service × 5 conn + 测试线程并发，默认 151 不够 |
| 启动超时 | 60s 轮询 `mysqladmin ping` | MySQL init 慢，第一次启动需 20–30s |
| Redis / RabbitMQ | 不动 | iam / audit / notification 仍各自起 testcontainers；只共享 MySQL |
| 默认开关 | `--shared-mysql` 默认开 | 用户主诉是「慢」；`--no-shared-mysql` 留作回归 / 调试 |
| cleanup 算法 | 简化为 `docker rm -f coder-test-mysql` | 不再需要 50 行 diff 算法识别本次新生成的容器 |

## 改动清单

### 1. `scripts/run-tests-docker.js`（核心）

- 新增常量：`MYSQL_CONTAINER_NAME = 'coder-test-mysql'`、`MYSQL_IMAGE = 'mysql:8.0'`
- 新增函数：
  - `startSharedMysql()`：拉镜像（best-effort 已存在跳过）→ `docker rm -f coder-test-mysql` → `docker run -d --name coder-test-mysql --network host --max-connections=500 --innodb-use-native-aio=0` → 轮询 `docker exec coder-test-mysql mysqladmin ping -uroot --port=<port>` 直到 OK（≤60s）
  - `stopSharedMysql()`：`docker rm -f coder-test-mysql`
- `pickTargets` 旁边加 `pickSharedMode(args)`：默认 true，遇 `--no-shared-mysql` 关闭
- `runTests(svc)`：根据 shared 模式决定是否向 `docker run` 注入 `-e TEST_MYSQL_HOST=127.0.0.1 -e TEST_MYSQL_PORT=<port>`
- `main()`：
  - shared 模式：先 `startSharedMysql()`，再跑测试，最后 `stopSharedMysql()`
  - 非 shared 模式：保留原 `listTestContainerIds` + `cleanupNewContainers` diff 算法
- `process.on('exit' / 'SIGINT' / 'SIGTERM')` 钩子：shared 模式下走 `stopSharedMysql`

### 2. `services/<svc>/tests/`（helper 改造）

每个 service 的 `tests/common/mod.rs`（已有或新建）提供 `spawn_mysql()`：

```rust
pub async fn spawn_mysql() -> (DbPool, Option<ContainerAsync<Mysql>>) {
    if let (Ok(host), Ok(port)) = (
        std::env::var("TEST_MYSQL_HOST"),
        std::env::var("TEST_MYSQL_PORT"),
    ) {
        // shared 模式：连共享 MySQL，建独立 database
        let port: u16 = port.parse().expect("TEST_MYSQL_PORT");
        let svc = env!("CARGO_PKG_NAME");
        let pid = std::process::id();
        let rand: u32 = rand::random();
        let db = format!("test_{svc}_{pid}_{rand:x}");

        let admin_url = format!("mysql://root@{host}:{port}/");
        let admin = MySqlPool::connect(&admin_url).await.expect("admin connect");
        sqlx::query(&format!("CREATE DATABASE `{db}` CHARACTER SET utf8mb4"))
            .execute(&admin).await.expect("create db");
        admin.close().await;

        let url = format!("mysql://root@{host}:{port}/{db}");
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url).await.expect("pool connect");
        run_migrations(&pool).await;
        (DbPool::Mysql(pool), None)
    } else {
        // fallback：原 testcontainers 起容器，保留本地 cargo test 路径
        let c = Mysql::default()
            .with_cmd(["--innodb-use-native-aio=0"])
            .start().await.unwrap();
        let host = c.get_host().await.unwrap();
        let port = c.get_host_port_ipv4(3306).await.unwrap();
        let url = format!("mysql://root@{host}:{port}/test");
        let pool = MySqlPool::connect(&url).await.unwrap();
        run_migrations(&pool).await;
        (DbPool::Mysql(pool), Some(c))
    }
}
```

具体改动按 service：

| Service | 改动 |
|---------|------|
| token-service | 改 `tests/common/mod.rs::spawn_mysql()` 加 shared 分支（已存在 helper） |
| iam-service | 新建 `tests/common/mod.rs`；8 个 test 文件 `setup()` 内的 5 行 Mysql 启动样板 → `mod common;` + `common::spawn_mysql().await` |
| notification-service | 新建 `tests/common/mod.rs`；2 个 test 文件（`announcement_publish_test.rs`、`announcement_recipient_test.rs`）改用 helper |

iam-service 的 `mod common;` 注意：现有 11 个 test 文件里 8 个用 MySQL、3 个不用（`change_password_test.rs`、`integration_test.rs`、`swagger_test.rs`）。8 个加 `mod common;`，3 个保持不变（不影响 cargo build）。

## 验证方案

1. **基础**：`node scripts/run-tests-docker.js iam-service` —— 应该只看到一个 mysql 容器（`docker ps | grep mysql`）
2. **隔离**：`docker exec coder-test-mysql mysql -uroot -e "SHOW DATABASES"` —— 应能看到 `test_iam-service_<pid>_<rand>`
3. **耗时对比**：`time node scripts/run-tests-docker.js iam-service` vs `--no-shared-mysql` 版本
4. **全量**：`node scripts/run-tests-docker.js` —— 仍然只有一个 `coder-test-mysql`，全部 service 通过
5. **本地 fallback**：`cargo test -p iam-service` 不经过 docker 时，仍能起本地 testcontainers 容器（开发体验不退化）
6. **CI**：本地无 Docker 时 fallback 分支必走，CI 仍能跑

## 风险与取舍

| 风险 | 缓解 |
|------|------|
| `max_connections` 151 不够并发 | mysql 启动加 `--max-connections=500` |
| 并发 service 抢同一 database name | db 名带 `pid` + `rand`，不会冲突 |
| 跨 service 全局状态干扰（如 `FLUSH` / `SHOW VARIABLES`） | 实际代码无此类操作；如出现再加 `SESSION` 隔离 |
| iam-service 8 处重复 setup 是历史债 | 借机抽 `tests/common/mod.rs`；3 个非 MySQL 测试不动 |
| `--no-shared-mysql` 路径保留 | 50 行 diff cleanup 算法不删，加分支判断 |
| Redis / RabbitMQ 仍各起 | 收益边际递减、复杂度上升，不动 |

## 不做

- 不引入 `common-test` crate —— 抽到 8 个 service 复制粘贴即可，3 处重复不算债
- 不动 service 主代码
- 不改 MySQL 镜像版本 / database 默认 schema
- 不动 `tests/common/mod.rs` 之外的 service 测试逻辑

## 回滚

`--no-shared-mysql` 一行 flag 回退到原行为。