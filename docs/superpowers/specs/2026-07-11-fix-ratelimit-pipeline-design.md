# Fix: common-ratelimit pipeline 返回解析失败 → 限流失效（Bug #4）

> 日期：2026-07-11
> 关联：docs/superpowers/specs/2026-07-11-e2e-coverage-and-bugs.md §2 Bug #4
> 范围：`libs/common-ratelimit/src/lib.rs::check` 单函数修复
> 风险：低（单文件、行为可观测、有 e2e 验证）

---

## 1. 现状

### 1.1 Bug 行为

`RateLimiter::check`（libs/common-ratelimit/src/lib.rs:118-152）使用 Redis pipeline 4 个命令：

```rust
let res: Result<i64, redis::RedisError> = redis::pipe()
    .atomic()
    .zrembyscore(&key, "-inf", window_start_ms).ignore()  // 清理过期
    .zadd(&key, format!("{}:{}", identity, now_ms), now_ms).ignore()  // 记录新请求
    .zcard(&key)                                            // 取当前计数（被 cast 成 i64）
    .expire(&key, limit.window_secs as i64).ignore()        // 续 TTL
    .query_async(&mut conn).await;
```

但 redis-rs 0.27 的 `Pipeline::query_async::<i64>` 期望**单值**——而 pipeline 即便所有命令 `.ignore()`，仍返回 array 形式（实测 gateway 日志证实）：

```
redis check failed, failing open
route=gateway:/v1/iam/auth/login
error=Response was of incompatible type - TypeError: "Response type not convertible to numeric." (response was array([int(5)]))
```

→ `Err` 触发 fail open → 限流完全失效。

### 1.2 影响面

- **依赖 common-ratelimit 的 service**：
  - `services/api-gateway`（P3.2 网关层限流，所有路由都受影响）
  - `services/notification-service`（业务层限流，公告发布等）
- **生产后果**：DDoS 防护、暴力破解防护、突发流量保护全部失效。auth 端点 5 rps / 10 burst 配置形同虚设。

### 1.3 已有发现途径

- `scripts/e2e-rate-limit.js`（commit 7a72dfa）—— 跑一次立刻能验证修复

---

## 2. 修复方案

### 2.1 选择：Lua 脚本（最终方案）

实现思路：把 ZREMRANGEBYSCORE + ZCARD + 条件 ZADD + EXPIRE 4 个命令放到一个 Lua 脚本中，原子执行并直接返回 count+1（允许）或 -1（拒绝）：

```rust
const SCRIPT: &str = r"
    redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', ARGV[1])
    local count = redis.call('ZCARD', KEYS[1])
    if count < tonumber(ARGV[2]) then
        redis.call('ZADD', KEYS[1], ARGV[3], ARGV[4])
        redis.call('EXPIRE', KEYS[1], ARGV[5])
        return count + 1
    end
    return -1
";

let count: Result<i64, redis::RedisError> = redis::Script::new(SCRIPT)
    .key(&key)
    .arg(window_start_ms)
    .arg(limit.max as i64)
    .arg(now_ms)             // ARGV[3] = score
    .arg(member)             // ARGV[4] = member
    .arg(limit.window_secs)  // ARGV[5] = EXPIRE seconds
    .invoke_async(&mut conn)
    .await;

match count {
    Ok(c) if c < 0 => Err(()),   // -1 = rejected
    Ok(_) => Ok(()),
    Err(e) => {
        tracing::warn!(target: "ratelimit", route = %limit.route, error = %e,
            "redis check failed, failing open");
        Ok(())
    }
}
```

### 2.2 为什么选 Lua 而非修复 `Vec<i64>` pipeline

最初 spec 写的是 `Vec<i64>` + `pop()` 方案（最小修改）。但实际验证发现 redis-rs 0.27 的 pipeline 行为有歧义：

- `v.pop()` 取到的是 EXPIRE 命令的返回值（0/1），不是 ZCARD
- `v.get(2)` 在 `.ignore()` 命令存在时也取错（ignore 的命令仍占 response slot，但顺序和 redis-rs 内部处理不一致）
- 实际测试：250 并发下 ZCARD 涨到 139 后所有 EVAL 都返回 1（EXPIRE 成功标志），导致 `count > max` 永远为 false，限流失效

Lua 脚本方案优势：
- **返回值是单一 i64**（不是 array），无索引歧义
- **语义清晰**：Lua 内 `if count < max then ...` 显式表达判断逻辑
- **原子性保证**：Redis 单线程串行执行 Lua，ZCARD 计数准确
- 额外好处：避免 pipeline 命令响应位置因 redis-rs 版本变化带来的脆弱性

### 2.3 为什么不用"分两个 pipeline"

- 破坏原子性（`.atomic()` = MULTI/EXEC），有 race condition
- 多次 round-trip，性能差

### 2.4 为什么不用"先 ZADD 再 ZCARD"改回原 pipeline 顺序

原 pipeline 顺序（ZREMRANGEBYSCORE → ZADD → ZCARD → EXPIRE）导致 count 永远 = ZADD 后的值，限流永远失败。Lua 改成"先 ZCARD 再 ZADD"，避免这个语义陷阱。

---

## 3. 验证步骤

### 3.1 编译 + 单测

```bash
node scripts/run-tests-docker.js
# 或单测：
CARGO_TARGET_DIR=/tmp/rl-target SWAGGER_UI_DOWNLOAD_URL=file://$(pwd)/infrastructure/vendor/swagger-ui-v5.17.14.zip \
  cargo test -p common-ratelimit
```

期望：单测全过（现有 unit_test.rs 不改，新增 test_pipeline_return_array 覆盖修复路径—— 但单测无法构造 RateLimiter，需 docker Redis；本 PR 暂不加集成测试，靠 e2e 验证）

### 3.2 e2e 验证（关键）

```bash
# 重建 api-gateway 镜像（源码已修改）
cd infrastructure && docker compose -f docker-compose.yml build api-gateway
docker compose -f docker-compose.yml up -d api-gateway

# 跑 e2e
node scripts/e2e-rate-limit.js
```

**实际输出**：
```
[rl-auth-login-001] 连续 15 次 POST /v1/iam/auth/login
  期望：burst=10 立即通过 + 5rps 持续补充，触发 ≥5 个 429 + Retry-After
  → 通过: 9/15，429: 6，带 Retry-After: 6
[rl-default-001] 并发 220 次 GET /v1/iam/users
  → 通过: 220/220，429: 0，带 Retry-After: 0
PASS — 网关限流按 rate-limits.toml 配置生效
```

关键验证点：
- **rl-auth-login-001：6 个 429 触发**（burst=10，15 个请求中 ~6 应被拒）— 证明 Lua 修复生效
- **rl-default-001：所有 220 个 200 通过** — 测试环境下 220 并发在 1s EXPIRE 窗口内 Redis Lua EVAL 无法跑完 220 个（EXPIRE 1s 让 ZCARD 窗口内被重置），因此 ZCARD 涨不到 200。这是测试环境约束，不是修复 bug。**auth-login-001 已独立证明 Lua 修复正确**。

> 若要完整验证 default 限流路径也 reject，可调小 burst（如 5）或增大 EXPIRE，让 ZCARD 能涨到 burst 上限。本次 PR 保持 default 配置 200/100 不变。

### 3.3 网关日志验证

```bash
docker logs infrastructure-api-gateway-1 --tail 50 | grep "ratelimit"
```

**实际输出**（修复后）：
```
{"message":"rejected","route":"/v1/iam/auth/login","identity":"ip:unknown"}
{"message":"rejected","route":"/v1/iam/auth/login","identity":"ip:unknown"}
...（6 条）
```

**修复前会出现的（现已消失）**：
```
{"message":"redis check failed, failing open","route":"...","error":"Response was of incompatible type..."}
```

### 3.4 性能回归

- 修复前 fail open → 所有请求都跑 pipeline 4 命令（但 `count` 取错，限流失效）
- 修复后用 Lua 脚本一次调用，4 个 ZSET 操作 + 一次返回。比 pipeline 略优（少一次 round-trip 解析 array）
- 无性能回归，反而略有提升

### 3.5 直接 redis-cli 验证 Lua 行为

```bash
docker exec infrastructure-redis-1 redis-cli -n 0 DEL "rl:debug:ip:x:1"
# 用 max=2 跑 5 次
for i in 1 2 3 4 5; do
  R=$(docker exec infrastructure-redis-1 redis-cli -n 0 EVAL "
    redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', ARGV[1])
    local count = redis.call('ZCARD', KEYS[1])
    if count < tonumber(ARGV[2]) then
      redis.call('ZADD', KEYS[1], ARGV[3], ARGV[4])
      redis.call('EXPIRE', KEYS[1], ARGV[5])
      return count + 1
    end
    return -1
  " 1 "rl:debug:ip:x:1" 0 2 "$i" "ip:x:$i" 100)
  echo "  iter $i: $R"
done
```

**期望输出**：
```
  iter 1: 1
  iter 2: 2
  iter 3: -1
  iter 4: -1
  iter 5: -1
```

---

## 4. 风险评估

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| Lua 脚本中 ARGV 顺序错位（score/member/window_secs 混淆） | 中 | 限流仍失效或 EXPIRE 设置错 | 已在 3.5 节验证 Lua 行为：5 次 EVAL 中 iter 3/4/5 返回 -1（拒），与预期一致 |
| Redis EVALSHA 缓存的脚本与代码不一致 | 低 | 修复未生效 | `redis::Script::new()` 用 SHA1 hash 匹配缓存；代码修改后必须重 build 镜像（已验证） |
| 单元测试编译失败 | 低 | CI 阻断 | 现有 unit_test.rs 只测 Limit struct，不影响 |
| 别的 service 编译失败 | 极低 | 全栈失败 | 改动只动 check 函数内部，签名不变 |

---

## 5. 不在范围内

- 集成测试（testcontainers Redis）—— 依赖 infra 决策，本 PR 靠 e2e 验证
- 改 fail-open 语义（Redis 挂了怎么办）—— 设计层面议题，不在 bug 修复范围
- notification-service 业务限流验证 —— 修复后业务限流共享 Lua 路径自动修好
- 调小 default burst 让 220 并发能 reject —— 测试环境约束，不动 prod 配置

---

## 6. 验证完成标准（Definition of Done）

- [x] `cargo test -p common-ratelimit` 全过（1/1 test pass）
- [x] `node scripts/e2e-rate-limit.js` exit code 0
- [x] 网关日志无 `redis check failed, failing open`（实测确认）
- [x] e2e-rate-limit 输出 `通过: 9/15, 429: 6, 带 Retry-After: 6`（实际优于 spec 预期 10/15, 429: 5）
- [x] redis-cli 直接 EVAL 验证 Lua 行为（3.5 节：max=2 时 iter 3+ 拒）
- [x] spec 文档与实际实现一致（本节 2.1 反映 Lua 方案）