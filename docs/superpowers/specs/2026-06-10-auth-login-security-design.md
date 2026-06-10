# Auth Service 登录安全策略 — 设计

日期：2026-06-10
状态：待审阅
服务：services/auth-service

## 目标

为密码账号登录增加防爆破与审计能力：

1. 连续失败 N 次后锁定账户一段时间，到期自动解锁。
2. 账号不存在也走相同规则，避免被用来枚举用户名。
3. 记录每次登录尝试（成功 / 失败）的 IP、User-Agent、尝试的账号、时间。
4. 记录每个用户的最近一次成功登录时间与 IP。

## 设计决策摘要

| 决策 | 选择 | 理由 |
|------|------|------|
| 计数器存储 | Redis | 天然 TTL、原子 INCR、低延迟 |
| 计数 key 维度 | `auth:login:fail:<username>` | 账号存在与否都计数；防枚举 |
| 锁 key 维度 | `auth:login:lock:<username>` | 与计数 key 分离，独立 TTL |
| 锁定响应 | HTTP 423 + `retry_after` | 语义清晰，前端可直接提示重试时间 |
| 日志表 | 复用 `usr_login_log` + 新增字段 | 成功失败一表查询，结构紧凑 |
| 配置位置 | `config/config.yaml` 的 `login_security` 段 | 与现有配置风格一致 |
| 默认值 | `max_failures=6`、`lockout_duration=5m`、`counter_window=15m` | 与需求一致；窗口 ≥ 锁时长 |

## 配置

在 `config/config.yaml` 中新增：

```yaml
login_security:
  max_failures: 6        # 失败阈值
  lockout_duration: 5m   # 锁定时长
  counter_window: 15m    # 失败计数器 TTL，必须 >= lockout_duration
```

在 `config/config.go` 中新增 `LoginSecurityConfig`：

```go
type LoginSecurityConfig struct {
    MaxFailures    int           `mapstructure:"max_failures"`
    LockoutDuration time.Duration `mapstructure:"lockout_duration"`
    CounterWindow  time.Duration `mapstructure:"counter_window"`
}
```

`Config` 增字段 `LoginSecurity LoginSecurityConfig`。
`Load` 中若未设置则填默认值（6 / 5min / 15min）。

## 数据模型

### `usr_user` 扩展

```go
LastLoginAt *time.Time `gorm:"index" json:"last_login_at"`
LastLoginIP string     `gorm:"size:45" json:"last_login_ip"`
```

### `usr_login_log` 扩展

```go
type LoginLog struct {
    ID         uint      `gorm:"primaryKey"`
    UserID     *uint     `gorm:"index"`         // 失败且账号不存在时为 nil
    Username   string    `gorm:"size:64;index"` // 记录尝试的账号
    IP         string    `gorm:"size:45"`
    UserAgent  string    `gorm:"size:255"`
    Success    bool      `gorm:"index"`
    FailReason string    `gorm:"size:32"`       // user_not_found | bad_password | locked
    LoginAt    time.Time `gorm:"index"`
    CreatedAt  time.Time
}
```

迁移通过 `db.AutoMigrate(&models.User{}, &models.LoginLog{})` 自动加列。

## 登录流程

```
POST /api/auth/login
        │
        ▼
1. 解析并校验 body（username, password 必填）
        │
        ▼
2. 检查 auth:login:lock:<username>
   - 存在 → 写 LoginLog(success=false, fail_reason=locked)
         → 返回 423 + {"error":"account locked","retry_after":<TTL 剩余秒>}
        │
        ▼
3. 查 User (by username)
   - 不存在 → 与错误密码路径走相同流程：
     * bcrypt 比较一个 dummy hash（耗时对齐）
     * 走步骤 5 失败分支
        │
        ▼
4. bcrypt 校验密码
   - 失败 → 步骤 5
   - 成功 → 步骤 6
        │
        ▼
5. 失败分支：
   a) n := INCR auth:login:fail:<username>
      若 n == 1 → EXPIRE 该 key 为 counter_window
   b) 若 n >= max_failures → SET auth:login:lock:<username> 1 EX lockout_duration
   c) 写 LoginLog(success=false, fail_reason=user_not_found|bad_password, user_id=?, username=<u>, ip, ua)
   d) 返回 401 {"error":"invalid credentials"}
        │
        ▼
6. 成功分支：
   a) DEL auth:login:fail:<username>
   b) DEL auth:login:lock:<username>（防御性，正常情况下应已过期）
   c) 更新 User.LastLoginAt = now, User.LastLoginIP = ip（gorm Updates）
   d) 写 LoginLog(success=true, user_id, username, ip, ua, login_at=now)
   e) 生成 auth code（原有逻辑）
   f) 返回 200 {"code":..., "expires_in":300}
```

## Redis key 设计

| Key | 类型 | TTL | 用途 |
|-----|------|-----|------|
| `auth:login:fail:<username>` | int | `counter_window` | 失败计数 |
| `auth:login:lock:<username>` | string "1" | `lockout_duration` | 锁定标记 |
| `auth:code:<code>` | int (userID) | 5min | 原有登录后 code |

`<username>` 为原始请求体内容，不做大小写归一化（避免绕过）。
若未来需要大小写不敏感登录，应在归一化后再设 key。

## 错误码

| 场景 | HTTP | body |
|------|------|------|
| 缺少字段 | 400 | `{"error":"<gin 校验错误>"}` |
| 账号被锁 | 423 | `{"error":"account locked","retry_after":<秒>}` |
| 账号或密码错 | 401 | `{"error":"invalid credentials"}` |
| 成功 | 200 | `{"code":"...","expires_in":300}` |

被锁和密码错区分状态码，但错误文案保持模糊，不暴露账号是否存在。

## IP 与代理

`c.ClientIP()` 自动按 `engine.SetTrustedProxies` 配置获取。
`main.go` 启动时配置 `engine.SetTrustedProxies([]string{"0.0.0.0/0"})` 由反向代理层保证安全。
若不可信，改为 `[]string{}` 仅取直连 IP。

## 文件改动清单

- `services/auth-service/config/config.go` — 新增 `LoginSecurityConfig` 与默认填充
- `services/auth-service/config/config.yaml` — 新增 `login_security` 段
- `services/auth-service/models/user.go` — `User` 加 `LastLoginAt` / `LastLoginIP`；`LoginLog` 加 `Username` / `UserID*` / `Success` / `FailReason`
- `services/auth-service/handlers/auth.go` — 改造 `Login`；新增内部辅助 `recordAttempt` / `isLocked` / `incrFailure` / `clearFailure` / `writeLog`
- `services/auth-service/main.go` — `engine.SetTrustedProxies` 配置
- `services/auth-service/docker/config/auth-service/config.yaml` — 同步新增段
- `services/auth-service/handlers/auth_test.go` — 增加失败计数、锁定、清零、日志写入的单元 + 集成测试

## 测试策略

`auth_test.go` 已有 `skipIfRedisUnavailable` 模式。补充：

1. **单元（无 DB 无 Redis）**：
   - 解析失败仍 400
   - bcrypt dummy hash 比较耗时存在
2. **集成（连真实 Redis 与 MySQL）**：
   - 失败 6 次 → 第 7 次 423
   - 成功 1 次后计数器清零
   - 账号不存在也参与计数
   - LoginLog 表记录成功与失败
   - User.LastLoginAt / LastLoginIP 在成功后更新

每个集成测试用例使用独立 username + Redis DB 1，避免污染。

## 不做的事

- 不做 IP 维度的限流（防同一 IP 多账号爆破不在本设计内）
- 不做验证码 / 二次认证
- 不做滑动窗口或指数退避（固定计数 + 锁即可）
- 不引入新的中间件或库（沿用 gin + redis/v9 + gorm）

## 风险与注意

- 跨实例部署：Redis 是单点事实来源，无需在内存缓存。
- Redis 重启：未到 TTL 的计数丢失，最坏情况是用户能多试一次。可接受。
- 日志写入失败：失败路径用 `db.Create` 同步写，错误记 `zap.Error` 但不影响登录返回。
- 时区：LoginAt 与 LastLoginAt 用 `time.Now()`，GORM DSN 已设 `loc=Local`，与配置一致。
