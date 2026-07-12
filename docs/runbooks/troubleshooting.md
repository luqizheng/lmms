# 综合故障排查 Runbook

> **状态**：草案 · 待评审
> **关联**：ai-context.md §6 runbooks 待推进项
> **适用范围**：coder-micro-services 平台生产环境故障排查

---

## 0. 一句话

> 按照「症状 → 诊断 → 处理」的顺序组织，优先快速止血，再查根因。

---

## 1. 快速诊断命令

### 1.1 基础设施健康检查

```bash
# 查看所有容器状态
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

# 期望输出：所有服务容器状态为 "Up"，无 "Restarting" 或 "Exited"

# 检查基础设施
docker ps | grep -E "mysql|redis|consul|rabbitmq"

# 查看资源使用
docker stats --no-stream
```

### 1.2 服务健康检查

```bash
# 各服务健康端点
curl -fs http://localhost:3000/health    # api-gateway
curl -fs http://localhost:8082/health    # token-service
curl -fs http://localhost:8081/health    # iam-service
curl -fs http://localhost:8083/health    # notification-service
curl -fs http://localhost:8084/health    # audit-service
curl -fs http://localhost:8085/health    # prue_backend
curl -fs http://localhost:8086/health    # file-service
curl -fs http://localhost:8087/health    # config-service

# 期望：所有返回 {"status":"ok"} 或类似
```

### 1.3 日志快速查看

```bash
# 查看最近错误日志
docker logs --tail 100 <container> 2>&1 | grep -E "ERROR|FATAL|panic"

# 实时跟踪日志
docker logs -f <container>

# 查看特定 trace_id
docker logs <container> 2>&1 | grep "0af7651916cd43dd8448eb211c80319c"
```

---

## 2. 服务级别问题

### 2.1 api-gateway（端口 3000）

#### 返回 401/403 Unauthorized

```bash
# 1. 检查 JWT 验签
curl -i http://localhost:3000/v1/iam/health

# 2. 检查 JWKS 是否可获取
curl http://localhost:8082/.well-known/jwks.json | jq .

# 3. 检查 Redis（JWT 黑名单）
docker exec redis redis-cli ping
docker exec redis redis-cli GET "blacklist:<token_jti>"

# 4. 查看日志
docker logs api-gateway 2>&1 | grep -i "jwt\|auth\|401"
```

**常见原因**：
- Token 过期（exp 已过）
- Token 在黑名单中（被登出/吊销）
- JWT 格式错误
- JWKS 未正确加载

#### 返回 429 Too Many Requests

```bash
# 1. 查看限流配置
cat infrastructure/config/api-gateway/rate-limits.toml

# 2. 检查 Redis 连接（限流计数存储在 Redis）
docker exec redis redis-cli ping

# 3. 查看限流日志
docker logs api-gateway 2>&1 | grep -i "rate.limit\|429"
```

#### 返回 502/504 Bad Gateway

```bash
# 1. 检查下游服务是否存活
curl -fs http://localhost:8081/health   # iam-service
curl -fs http://localhost:8082/health   # token-service

# 2. 检查 Consul 服务注册
curl http://localhost:8500/v1/health/service/iam-service

# 3. 检查网络连通性
docker exec api-gateway curl -v http://localhost:8081/health
```

### 2.2 token-service（端口 6001/8082）

#### Token 生成失败

```bash
# 1. 检查 MySQL 连接
docker exec token-service ping-mysql || docker logs token-service 2>&1 | grep -i mysql

# 2. 检查 Redis 连接
docker exec redis redis-cli ping

# 3. 检查 RS256 密钥文件
docker exec token-service ls -la /app/keys/

# 4. 查看详细日志
docker logs token-service 2>&1 | grep -i "token\|sign\|error"
```

#### JWKS 端点不可达

```bash
# 1. 直接访问 JWKS
curl http://localhost:8082/.well-known/jwks.json

# 2. 检查密钥是否存在
docker exec token-service cat /app/keys/public.pem

# 3. 重启 token-service（如密钥已更新）
docker compose -f infrastructure/docker-compose-services.yml restart token-service
```

### 2.3 iam-service（端口 6002/8081）

#### 登录失败

```bash
# 1. 检查请求格式
curl -X POST http://localhost:8081/v1/iam/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"wrongpassword"}'

# 2. 查看数据库连接
docker logs iam-service 2>&1 | grep -i "database\|pool"

# 3. 检查用户是否存在
docker exec mysql mysql -uroot -prootpass -e \
  "SELECT id, username FROM superpowers.iam_users LIMIT 5;"

# 4. 检查密码哈希算法
docker logs iam-service 2>&1 | grep -i "password\|hash\|argon"
```

#### 权限错误（403 Forbidden）

```bash
# 1. 检查用户角色
curl -H "Authorization: Bearer <token>" \
  http://localhost:8081/v1/iam/users/me | jq .

# 2. 检查权限码是否正确
# 需要的权限 vs 用户实际权限
grep -r "CFG_CONFIG_REVEAL" services/config-service/src/

# 3. 查看权限变更日志（RabbitMQ 事件）
docker logs iam-service 2>&1 | grep -i "permission"
```

### 2.4 notification-service（端口 6003/8083）

#### WebSocket 连接失败

```bash
# 1. 检查 WS 端点
curl -i -N \
  -H "Upgrade: websocket" \
  -H "Connection: Upgrade" \
  -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
  -H "Sec-WebSocket-Version: 13" \
  http://localhost:8083/v1/notification/ws

# 2. 检查 Redis 连接（WS 状态存储）
docker exec redis redis-cli ping

# 3. 查看 WS 日志
docker logs notification-service 2>&1 | grep -i "websocket\|ws\|upgrade"
```

#### 公告未推送

```bash
# 1. 检查 RabbitMQ 连接
docker exec notification-service \
  rabbitmq-diagnostics -q ping || echo "RabbitMQ connection failed"

# 2. 检查事件消费者
docker logs notification-service 2>&1 | grep -i "event\|consume\|rabbitmq"

# 3. 查看 ntf_outbox_events 表
docker exec mysql mysql -uroot -prootpass -e \
  "SELECT * FROM superpowers.ntf_outbox_events ORDER BY created_at DESC LIMIT 10;"
```

---

## 3. 基础设施问题

### 3.1 MySQL

#### 连接拒绝

```bash
# 1. 检查 MySQL 容器
docker ps | grep mysql
docker logs mysql 2>&1 | tail -50

# 2. 测试连接
docker exec mysql mysql -uroot -prootpass -e "SELECT 1;"

# 3. 检查连接数
docker exec mysql mysql -uroot -prootpass -e \
  "SHOW STATUS LIKE 'Threads_connected';"

# 4. 如果连接数过多
# 查看各服务的连接池配置
grep -r "pool_size\|max_connections" services/*/src/
```

#### 表不存在

```bash
# 1. 检查迁移状态
curl http://localhost:8081/internal/migrations/state

# 2. 查看已存在的表
docker exec mysql mysql -uroot -prootpass -e \
  "SHOW TABLES FROM superpowers;" | grep -E "iam_|audit_|token_"

# 3. 手动执行迁移
docker compose -f infrastructure/docker-compose-services.yml restart <svc>
docker logs <svc> 2>&1 | grep -i migration
```

#### 死锁

```bash
# 1. 查看死锁日志
docker exec mysql mysql -uroot -prootpass -e \
  "SHOW ENGINE INNODB STATUS\G" | grep -A 20 "LATEST DETECTED DEADLOCK"

# 2. 查看当前锁
docker exec mysql mysql -uroot -prootpass -e \
  "SELECT * FROM information_schema.INNODB_LOCKS;"

# 3. 查看事务
docker exec mysql mysql -uroot -prootpass -e \
  "SELECT * FROM information_schema.INNODB_TRX;"
```

### 3.2 Redis

#### 连接拒绝

```bash
# 1. 检查 Redis 容器
docker ps | grep redis
docker logs redis 2>&1 | tail -50

# 2. 测试连接
docker exec redis redis-cli ping

# 3. 查看内存使用
docker exec redis redis-cli INFO memory | grep used_memory_human
```

#### 内存压力（OOM）

```bash
# 1. 查看内存使用
docker exec redis redis-cli INFO memory | grep used_memory_human

# 2. 查看 big keys
docker exec redis redis-cli --bigkeys

# 3. 查看持久化
docker exec redis redis-cli INFO persistence

# 4. 如果 AOF 过大，可以暂时关闭
docker exec redis redis-cli CONFIG SET appendonly no
```

### 3.3 Consul

#### 服务未注册

```bash
# 1. 查看 Consul 服务列表
curl http://localhost:8500/v1/catalog/services

# 2. 查看特定服务健康检查
curl http://localhost:8500/v1/health/service/iam-service

# 3. 手动重新注册（通过 registrator）
# registrator 会自动注册，但可以强制刷新
docker restart registrator

# 4. 检查服务健康检查
curl http://localhost:8500/v1/agent/checks
```

### 3.4 RabbitMQ

#### 消息堆积

```bash
# 1. 查看队列
docker exec rabbitmq rabbitmqctl list_queues name messages consumers

# 2. 查看消费者状态
docker exec rabbitmq rabbitmqctl list_consumers

# 3. 查看死信队列
docker exec rabbitmq rabbitmqctl list_queues name messages | grep dlx

# 4. 如果是 outbox publisher 问题
docker logs iam-outbox-publisher 2>&1 | grep -i "poll\|publish"
```

---

## 4. 按 trace_id 排查

### 4.1 获取 trace_id

```bash
# 方式1：从响应头获取
curl -i http://localhost:3000/v1/iam/health | grep X-Trace-Id

# 方式2：从 Jaeger UI 获取
# 打开 http://localhost:16686 搜索
```

### 4.2 查 Jaeger

```bash
# 打开 trace 详情
http://localhost:16686/trace/<trace_id>

# 查看 span 详情，找到具体哪个服务/函数慢
```

### 4.3 查日志

```bash
# 用 trace_id 过滤所有相关日志
TRACE_ID="0af7651916cd43dd8448eb211c80319c"

for svc in api-gateway iam-service token-service notification-service; do
  echo "=== $svc ==="
  docker logs $svc 2>&1 | grep "$TRACE_ID"
done
```

---

## 5. 性能问题

### 5.1 服务响应慢

```bash
# 1. 查看 Prometheus 指标
# Prometheus: http://localhost:9090
# 查询：http_request_duration_seconds_bucket

# 2. 查看慢查询日志（MySQL）
docker exec mysql mysql -uroot -prootpass -e \
  "SHOW GLOBAL VARIABLES LIKE 'slow_query%';"
docker exec mysql mysql -uroot -prootpass -e \
  "SELECT * FROM mysql.slow_log ORDER BY start_time DESC LIMIT 10;"

# 3. 查看连接池状态
docker exec mysql mysql -uroot -prootpass -e \
  "SHOW STATUS LIKE 'Threads_connected';"
```

### 5.2 高 CPU/内存

```bash
# 1. 查看容器资源使用
docker stats --no-stream

# 2. 如果是 Rust 服务内存泄漏
# 查看内存是否持续增长
watch -n 5 'docker stats --no-stream | grep <svc>'

# 3. 查看具体进程的内存映射
docker exec <svc> cat /proc/1/maps
```

---

## 6. 部署问题

### 6.1 镜像拉取失败

```bash
# 1. 检查镜像是否存在
docker images | grep <svc>
docker pull zhcoder-docker-registry.com:8000/micro-server/<svc>:latest

# 2. 检查登录状态
docker login zhcoder-docker-registry.com:8000

# 3. 查看具体错误
docker pull <image> 2>&1
```

### 6.2 健康检查失败

```bash
# 1. 查看健康检查日志
docker inspect <container> | jq '.[0].State.Health'

# 2. 手动执行健康检查
docker exec <container> wget -qO- http://localhost:8080/health

# 3. 检查依赖服务
# 例如：iam-service 依赖 MySQL, Consul
```

### 6.3 迁移失败

```bash
# 1. 查看迁移日志
docker logs <svc> 2>&1 | grep -i "migration\|sqlx\|error"

# 2. 查看 migration-state 端点
curl http://localhost:8080/internal/migrations/state

# 3. 手动执行迁移
# 重启服务触发自动迁移
docker compose -f infrastructure/docker-compose-services.yml restart <svc>
```

---

## 7. 数据问题

### 7.1 查询 MySQL

```bash
# 连接 MySQL
docker exec -it mysql mysql -uroot -prootpass superpowers

# 查看表结构
SHOW TABLES LIKE 'iam_%';
DESCRIBE iam_users;

# 查看数据
SELECT COUNT(*) FROM iam_users;
SELECT * FROM iam_users LIMIT 5;

# 查看索引
SHOW INDEX FROM iam_users;

# 查看最近创建的表
SELECT table_name, create_time FROM information_schema.tables
WHERE table_schema = 'superpowers'
ORDER BY create_time DESC LIMIT 10;
```

### 7.2 操作 Redis

```bash
# 连接 Redis
docker exec -it redis redis-cli

# 基本操作
PING
KEYS *
GET <key>
TTL <key>

# 清除特定模式
FLUSHDB  # 慎用，清除所有

# 查看 JWT 黑名单
KEYS blacklist:*
```

---

## 8. 快速止血手册

### 8.1 服务无响应

```bash
# 1. 重启服务
docker compose -f infrastructure/docker-compose-services.yml restart <svc>

# 2. 如果还是不行，回滚版本
git checkout <previous-sha>
docker compose -f infrastructure/docker-compose-services.yml up -d --force-recreate <svc>

# 3. 检查是否是依赖问题
docker logs <svc> --tail 50
```

### 8.2 数据库连接问题

```bash
# 1. 重启 MySQL
docker compose -f infrastructure/docker-compose.yml restart mysql

# 2. 如果连接数满，等待自动清理
# 或手动清理
docker exec mysql mysql -uroot -prootpass -e \
  "SHOW PROCESSLIST;" | awk '{print $1}' | xargs -I{} docker exec mysql mysql -uroot -prootpass -e "KILL {};"
```

### 8.3 限流误伤

```bash
# 1. 临时调高限流阈值
vim infrastructure/config/api-gateway/rate-limits.toml

# 2. 热加载配置
curl -X POST http://localhost:3000/__admin/reload

# 3. 或临时禁用限流（不推荐用于生产）
# 修改配置后 reload
```

---

## 9. 关联

- `docs/onboarding/07-日志与排查Bug.md` — 新人日志排查
- `docs/onboarding/09-出问题怎么办.md` — 升级流程
- `docs/guides/otel-tracing.md` — 链路追踪
- `docs/runbooks/oncall.md` — On-call 值班

---

> **最后更新**：2026-07-13
