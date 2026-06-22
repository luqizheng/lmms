# Auth Service 登录安全策略实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** �?`auth-service` 中加入登录失败计�?+ 账户锁定 + 全量登录审计日志（成�?失败都记 IP、UA、尝试账号）�?

**Architecture:** 计数器与锁存�?Redis（独�?TTL），日志�?MySQL `usr_login_log` 扩展字段。失�?锁定响应分离�?01/423）但文案不暴露账号存在。配置项集中�?`config/config.yaml` �?`login_security` 段�?

**Tech Stack:** Go 1.21+ / Gin / go-redis v9 / GORM v2 / bcrypt / viper / zap

**Spec:** `docs/superpowers/specs/2026-06-10-auth-login-security-design.md`

---

## 文件结构

| 文件 | 责任 | 改动类型 |
|------|------|----------|
| `services/auth-service/config/config.go` | 配置结构与默认�?| 修改 |
| `services/auth-service/config/config.yaml` | 本地配置 | 修改 |
| `docker-example/config/auth-service/config.yaml` | Docker 部署配置 | 修改 |
| `services/auth-service/models/user.go` | User + LoginLog 模型 | 修改 |
| `services/auth-service/handlers/auth.go` | Login 流程与辅助函�?| 修改 |
| `services/auth-service/handlers/auth_test.go` | 单元 + 集成测试 | 修改 |
| `services/auth-service/main.go` | TrustedProxies 配置 | 修改 |

## 任务顺序

1. 配置：`LoginSecurityConfig` 与默认�?
2. 模型：`User` �?`LoginLog` 扩展
3. 处理器辅助：isLocked / incrFailure / clearFailure / recordLog
4. 处理器：Login 锁定拦截
5. 处理器：Login 失败计数 + 锁生�?
6. 处理器：Login 成功清零 + 写日�?
7. 处理器：账号不存在走相同失败路径
8. main.go：TrustedProxies
9. 集成测试：完整锁定流�?

---

### Task 1: 配置 �?`LoginSecurityConfig`

**Files:**
- Modify: `services/auth-service/config/config.go`
- Modify: `services/auth-service/config/config.yaml`
- Modify: `docker-example/config/auth-service/config.yaml`

- [ ] **Step 1: �?`config.go` 中新增结构体与默认�?*

�?`config.go` �?10-16 �?`type Config struct { ... }` 内增加字段：

```go
type Config struct {
    Server         ServerConfig         `mapstructure:"server"`
    Database       DatabaseConfig       `mapstructure:"database"`
    Redis          RedisConfig          `mapstructure:"redis"`
    TokenServer    TokenServerConfig    `mapstructure:"token_server"`
    Consul         ConsulConfig         `mapstructure:"consul"`
    LoginSecurity  LoginSecurityConfig  `mapstructure:"login_security"`
}
```

在第 47 �?`TokenServerConfig` 后新增：

```go
type LoginSecurityConfig struct {
    MaxFailures    int           `mapstructure:"max_failures"`
    LockoutDuration time.Duration `mapstructure:"lockout_duration"`
    CounterWindow  time.Duration `mapstructure:"counter_window"`
}

func (c *LoginSecurityConfig) ApplyDefaults() {
    if c.MaxFailures <= 0 {
        c.MaxFailures = 6
    }
    if c.LockoutDuration <= 0 {
        c.LockoutDuration = 5 * time.Minute
    }
    if c.CounterWindow <= 0 {
        c.CounterWindow = 15 * time.Minute
    }
}
```

- [ ] **Step 2: �?`Load` 中调用默认�?*

�?`config.go` �?89 �?`viper.Unmarshal(&cfg)` 之后加：

```go
cfg.LoginSecurity.ApplyDefaults()
if cfg.LoginSecurity.CounterWindow < cfg.LoginSecurity.LockoutDuration {
    return nil, fmt.Errorf("login_security.counter_window must be >= lockout_duration")
}
```

- [ ] **Step 3: 添加 `time` 导入**

```go
import (
    "fmt"
    "strings"
    "time"

    "github.com/spf13/viper"
)
```

- [ ] **Step 4: 更新两个 yaml 配置文件**

`config/config.yaml` �?`docker-example/config/auth-service/config.yaml` 末尾追加�?

```yaml
login_security:
  max_failures: 6
  lockout_duration: 5m
  counter_window: 15m
```

- [ ] **Step 5: 编译验证**

```bash
cd services/auth-service && go build ./...
```

期望：无错误�?

- [ ] **Step 6: 提交**

```bash
git add services/auth-service/config/ docker-example/config/auth-service/
git commit -m "feat(auth-service): login_security 配置段与默认�?
```

---

### Task 2: 模型 �?User �?LoginLog 扩展

**Files:**
- Modify: `services/auth-service/models/user.go`

- [ ] **Step 1: �?`User` 中新�?`LastLoginAt` �?`LastLoginIP`**

修改 `User` 结构（保持字段顺序在 Status 之后、CreatedAt 之前）：

```go
type User struct {
    ID           uint           `gorm:"primaryKey" json:"id"`
    Username     string         `gorm:"uniqueIndex;size:64;not null" json:"username"`
    PasswordHash string         `gorm:"size:255;not null" json:"-"`
    Email        string         `gorm:"size:128" json:"email"`
    Phone        string         `gorm:"size:20" json:"phone"`
    Nickname     string         `gorm:"size:64" json:"nickname"`
    Status       int8           `gorm:"default:1" json:"status"`
    LastLoginAt  *time.Time     `gorm:"index" json:"last_login_at"`
    LastLoginIP  string         `gorm:"size:45" json:"last_login_ip"`
    CreatedAt    time.Time      `json:"created_at"`
    UpdatedAt    time.Time      `json:"updated_at"`
    DeletedAt    gorm.DeletedAt `gorm:"index" json:"-"`
}
```

- [ ] **Step 2: 扩展 `LoginLog`**

替换 `LoginLog` 结构�?

```go
type LoginLog struct {
    ID         uint      `gorm:"primaryKey" json:"id"`
    UserID     *uint     `gorm:"index" json:"user_id"`
    Username   string    `gorm:"size:64;index" json:"username"`
    IP         string    `gorm:"size:45" json:"ip"`
    UserAgent  string    `gorm:"size:255" json:"user_agent"`
    Success    bool      `gorm:"index" json:"success"`
    FailReason string    `gorm:"size:32" json:"fail_reason"`
    LoginAt    time.Time `gorm:"index" json:"login_at"`
    CreatedAt  time.Time `json:"created_at"`
}

func (LoginLog) TableName() string {
    return "usr_login_log"
}
```

- [ ] **Step 3: 编译验证**

```bash
cd services/auth-service && go build ./...
```

期望：无错误�?

- [ ] **Step 4: 提交**

```bash
git add services/auth-service/models/user.go
git commit -m "feat(auth-service): User �?LastLoginAt/IP, LoginLog �?success/username/fail_reason"
```

---

### Task 3: 处理器辅�?�?Redis 计数与锁

**Files:**
- Modify: `services/auth-service/handlers/auth.go`

- [ ] **Step 1: 在文件顶部新增常量与包级变量**

�?`auth.go` �?17 �?`)` 后、`func Register` 前加�?

```go
const dummyBcryptHash = "$2a$10$N9qo8uLOickgx2ZMRZoMyeIjZAgcfl7p92ldGxad68LJZdL17lhWy" // "password"

var (
    failKeyPrefix = "auth:login:fail:"
    lockKeyPrefix = "auth:login:lock:"
)
```

- [ ] **Step 2: 新增 `failKey` / `lockKey` 帮助函数**

紧接在常量后�?

```go
func failKey(username string) string { return failKeyPrefix + username }
func lockKey(username string) string { return lockKeyPrefix + username }
```

- [ ] **Step 3: 新增 `isLocked`**

```go
func isLocked(ctx context.Context, rdb *redis.Client, username string) (bool, time.Duration, error) {
    ttl, err := rdb.TTL(ctx, lockKey(username)).Result()
    if err != nil {
        return false, 0, err
    }
    if ttl > 0 {
        return true, ttl, nil
    }
    return false, 0, nil
}
```

- [ ] **Step 4: 新增 `incrFailure`**

```go
func incrFailure(ctx context.Context, rdb *redis.Client, username string, max int, window time.Duration) (int64, bool, error) {
    n, err := rdb.Incr(ctx, failKey(username)).Result()
    if err != nil {
        return 0, false, err
    }
    if n == 1 {
        if err := rdb.Expire(ctx, failKey(username), window).Err(); err != nil {
            return n, false, err
        }
    }
    locked := n >= int64(max)
    return n, locked, nil
}
```

- [ ] **Step 5: 新增 `setLock`**

```go
func setLock(ctx context.Context, rdb *redis.Client, username string, d time.Duration) error {
    return rdb.Set(ctx, lockKey(username), "1", d).Err()
}
```

- [ ] **Step 6: 新增 `clearFailure`**

```go
func clearFailure(ctx context.Context, rdb *redis.Client, username string) {
    rdb.Del(ctx, failKey(username), lockKey(username))
}
```

- [ ] **Step 7: 编译验证**

```bash
cd services/auth-service && go build ./...
```

期望：无错误（即使这些函数尚未被使用）�?

- [ ] **Step 8: 提交**

```bash
git add services/auth-service/handlers/auth.go
git commit -m "refactor(auth-service): 登录安全辅助函数（isLocked/incrFailure/setLock/clearFailure�?
```

---

### Task 4: 处理�?�?Login 锁定拦截

**Files:**
- Modify: `services/auth-service/handlers/auth.go`
- Modify: `services/auth-service/handlers/auth_test.go`

- [ ] **Step 1: �?`Login` 顶部加锁定检�?*

修改 `Login` 函数体，�?`c.ShouldBindJSON` 之后、`db.DB.Where` 之前插入�?

```go
locked, retryAfter, err := isLocked(ctx, rdb, req.Username)
if err == nil && locked {
    db.DB.Create(&models.LoginLog{
        UserID:     nil,
        Username:   req.Username,
        IP:         c.ClientIP(),
        UserAgent:  c.GetHeader("User-Agent"),
        Success:    false,
        FailReason: "locked",
        LoginAt:    time.Now(),
    })
    c.JSON(http.StatusLocked, gin.H{
        "error":       "account locked",
        "retry_after": int(retryAfter.Seconds()),
    })
    return
}
```

�?`Login` 函数首行添加 `ctx := context.Background()`（供后续辅助使用）�?

- [ ] **Step 2: �?`auth_test.go` 添加失败测试**

�?`auth_test.go` 末尾添加（需�?`time` 已在 import 中）�?

```go
func TestLoginLockedReturns423(t *testing.T) {
    if testing.Short() {
        t.Skip("Skipping Redis integration test in short mode")
    }
    skipIfRedisUnavailable(t)
    rdb := getRedisClient()
    defer rdb.Close()
    ctx := context.Background()

    user := fmt.Sprintf("locked-test-%d", time.Now().UnixNano())
    require.NoError(t, rdb.Set(ctx, lockKey(user), "1", time.Minute).Err())
    defer rdb.Del(ctx, lockKey(user))

    cfg := &config.Config{}
    cfg.LoginSecurity.ApplyDefaults()
    router := setupTestRouter()
    router.POST("/login", Login(nil, rdb, cfg))

    body, _ := json.Marshal(map[string]string{
        "username": user,
        "password": "anything",
    })
    req := httptest.NewRequest(http.MethodPost, "/login", bytes.NewReader(body))
    req.Header.Set("Content-Type", "application/json")
    w := httptest.NewRecorder()
    router.ServeHTTP(w, req)

    assert.Equal(t, http.StatusLocked, w.Code)
    var resp map[string]interface{}
    require.NoError(t, json.Unmarshal(w.Body.Bytes(), &resp))
    assert.Equal(t, "account locked", resp["error"])
    assert.NotZero(t, resp["retry_after"])
}
```

�?`auth_test.go` �?import 块添加：

```go
import (
    "fmt"
    "superpowers/auth-service/config"
)
```

- [ ] **Step 3: 运行测试**

```bash
cd services/auth-service && go test ./handlers/ -run TestLoginLockedReturns423 -v
```

期望：PASS�?

- [ ] **Step 4: 提交**

```bash
git add services/auth-service/handlers/
git commit -m "feat(auth-service): Login 锁定时返�?423 + retry_after"
```

---

### Task 5: 处理�?�?Login 失败计数 + 自动�?

**Files:**
- Modify: `services/auth-service/handlers/auth.go`
- Modify: `services/auth-service/handlers/auth_test.go`

- [ ] **Step 1: 替换 `Login` 中“账号或密码错”分�?*

�?`Login` 中第 73-82 行（用户查询 + bcrypt 校验）替换为�?

```go
var user models.User
userErr := db.DB.Where("username = ?", req.Username).First(&user).Error

if userErr != nil {
    _ = bcrypt.CompareHashAndPassword([]byte(dummyBcryptHash), []byte(req.Password))
    n, _, ierr := incrFailure(ctx, rdb, req.Username,
        cfg.LoginSecurity.MaxFailures, cfg.LoginSecurity.CounterWindow)
    if ierr == nil && n >= int64(cfg.LoginSecurity.MaxFailures) {
        _ = setLock(ctx, rdb, req.Username, cfg.LoginSecurity.LockoutDuration)
    }
    c.JSON(http.StatusUnauthorized, gin.H{"error": "invalid credentials"})
    return
}

if err := bcrypt.CompareHashAndPassword([]byte(user.PasswordHash), []byte(req.Password)); err != nil {
    n, _, ierr := incrFailure(ctx, rdb, req.Username,
        cfg.LoginSecurity.MaxFailures, cfg.LoginSecurity.CounterWindow)
    if ierr == nil && n >= int64(cfg.LoginSecurity.MaxFailures) {
        _ = setLock(ctx, rdb, req.Username, cfg.LoginSecurity.LockoutDuration)
    }
    c.JSON(http.StatusUnauthorized, gin.H{"error": "invalid credentials"})
    return
}
```

- [ ] **Step 2: 添加 `incrFailure` 集成测试**

�?`auth_test.go` 末尾添加�?

```go
func TestIncrFailureSetsLockAtThreshold(t *testing.T) {
    if testing.Short() {
        t.Skip("Skipping Redis integration test in short mode")
    }
    skipIfRedisUnavailable(t)
    rdb := getRedisClient()
    defer rdb.Close()
    ctx := context.Background()

    user := fmt.Sprintf("fail-test-%d", time.Now().UnixNano())
    defer rdb.Del(ctx, failKey(user), lockKey(user))

    n, locked, err := incrFailure(ctx, rdb, user, 3, time.Minute)
    require.NoError(t, err)
    assert.Equal(t, int64(1), n)
    assert.False(t, locked)

    n, locked, err = incrFailure(ctx, rdb, user, 3, time.Minute)
    require.NoError(t, err)
    assert.Equal(t, int64(2), n)
    assert.False(t, locked)

    n, locked, err = incrFailure(ctx, rdb, user, 3, time.Minute)
    require.NoError(t, err)
    assert.Equal(t, int64(3), n)
    assert.True(t, locked)
}
```

- [ ] **Step 3: 运行测试**

```bash
cd services/auth-service && go test ./handlers/ -run TestIncrFailureSetsLockAtThreshold -v
```

期望：PASS�?

- [ ] **Step 4: 提交**

```bash
git add services/auth-service/handlers/
git commit -m "feat(auth-service): 登录失败 INCR 计数并按阈值自动锁"
```

---

### Task 6: 处理�?�?Login 成功清零 + 写日�?

**Files:**
- Modify: `services/auth-service/handlers/auth.go`
- Modify: `services/auth-service/handlers/auth_test.go`

- [ ] **Step 1: �?`Login` 成功路径插入清零与日�?*

�?`Login` �?`code := uuid.New().String()` 前一行替换为�?

```go
clearFailure(ctx, rdb, req.Username)

now := time.Now()
ip := c.ClientIP()
ua := c.GetHeader("User-Agent")
uid := user.ID

if err := db.DB.Create(&models.LoginLog{
    UserID:    &uid,
    Username:  req.Username,
    IP:        ip,
    UserAgent: ua,
    Success:   true,
    LoginAt:   now,
}).Error; err != nil {
    zap.L().Sugar().Errorf("login log write failed: %v", err)
}

db.DB.Model(&user).Updates(map[string]interface{}{
    "last_login_at": now,
    "last_login_ip": ip,
})

code := uuid.New().String()
```

- [ ] **Step 2: 添加 `clearFailure` 单元测试**

```go
func TestClearFailureRemovesKeys(t *testing.T) {
    if testing.Short() {
        t.Skip("Skipping Redis integration test in short mode")
    }
    skipIfRedisUnavailable(t)
    rdb := getRedisClient()
    defer rdb.Close()
    ctx := context.Background()

    user := fmt.Sprintf("clear-test-%d", time.Now().UnixNano())
    require.NoError(t, rdb.Set(ctx, failKey(user), 5, time.Minute).Err())
    require.NoError(t, rdb.Set(ctx, lockKey(user), "1", time.Minute).Err())

    clearFailure(ctx, rdb, user)

    failExists, _ := rdb.Exists(ctx, failKey(user)).Result()
    lockExists, _ := rdb.Exists(ctx, lockKey(user)).Result()
    assert.Equal(t, int64(0), failExists)
    assert.Equal(t, int64(0), lockExists)
}
```

- [ ] **Step 3: 运行测试**

```bash
cd services/auth-service && go test ./handlers/ -run TestClearFailure -v
```

期望：PASS�?

- [ ] **Step 4: 提交**

```bash
git add services/auth-service/handlers/
git commit -m "feat(auth-service): 登录成功后清�?+ 记录成功日志 + 更新 last_login_*"
```

---

### Task 7: 处理�?�?失败路径写日�?+ main �?logger

**Files:**
- Modify: `services/auth-service/handlers/auth.go`
- Modify: `services/auth-service/main.go`
- Modify: `services/auth-service/routes.go`

- [ ] **Step 1: 在失败分支写日志**

修改 `Login` 的两�?`c.JSON(StatusUnauthorized)` 之前，调用：

```go
reason := "bad_password"
if userErr != nil {
    reason = "user_not_found"
}
if err := db.DB.Create(&models.LoginLog{
    UserID:     nil,
    Username:   req.Username,
    IP:         c.ClientIP(),
    UserAgent:  c.GetHeader("User-Agent"),
    Success:    false,
    FailReason: reason,
    LoginAt:    time.Now(),
}).Error; err != nil {
    zap.L().Sugar().Errorf("login log write failed: %v", err)
}
```

注意：第二个失败分支（账号存在、密码错）里 `err` 来自 `bcrypt.CompareHashAndPassword`，把第一段推断的 `reason` 默认值改�?`"bad_password"` 即可，无需重新区分 userErr（该分支 userErr == nil）�?

- [ ] **Step 2: 添加 zap.L() 初始化到 main**

`main.go` �?`logger, _ := zap.NewProduction()` 之后加：

```go
zap.ReplaceGlobals(logger)
```

- [ ] **Step 3: routes.go 注入 zap**

修改 `setupRoutes` 顶部�?

```go
func (s *Server) setupRoutes() {
    if _, err := zap.NewProduction(); err == nil {
        zap.L().Info("zap already initialized")
    }
    s.engine.GET("/health", s.healthHandler)

    auth := s.engine.Group("/api/auth")
    {
        auth.POST("/register", handlers.Register(s.db))
        auth.POST("/login", handlers.Login(s.db, s.redis, s.cfg))
        auth.POST("/logout", handlers.Logout(s.redis, s.cfg))
    }
}
```

并加 import `"go.uber.org/zap"` �?routes.go�?

- [ ] **Step 4: 编译验证**

```bash
cd services/auth-service && go build ./...
```

期望：无错误�?

- [ ] **Step 5: 提交**

```bash
git add services/auth-service/handlers/ services/auth-service/main.go services/auth-service/routes.go
git commit -m "feat(auth-service): 失败登录记录 reason + zap.ReplaceGlobals"
```

---

### Task 8: 集成测试 �?完整锁定流程

**Files:**
- Modify: `services/auth-service/handlers/auth_test.go`

- [ ] **Step 1: 编写端到端测�?*

�?`auth_test.go` 末尾添加（使�?Redis DB 2 隔离）：

```go
func TestLoginEndToEndLockoutFlow(t *testing.T) {
    if testing.Short() {
        t.Skip("Skipping Redis integration test in short mode")
    }
    skipIfRedisUnavailable(t)
    rdb := getRedisClient()
    defer rdb.Close()
    ctx := context.Background()

    user := fmt.Sprintf("e2e-%d", time.Now().UnixNano())
    defer rdb.Del(ctx, failKey(user), lockKey(user))

    cfg := &config.Config{}
    cfg.LoginSecurity.MaxFailures = 3
    cfg.LoginSecurity.LockoutDuration = time.Minute
    cfg.LoginSecurity.CounterWindow = 2 * time.Minute

    router := setupTestRouter()
    router.POST("/login", Login(nil, rdb, cfg))

    body, _ := json.Marshal(map[string]string{
        "username": user,
        "password": "wrong",
    })

    for i := 1; i <= 3; i++ {
        req := httptest.NewRequest(http.MethodPost, "/login", bytes.NewReader(body))
        req.Header.Set("Content-Type", "application/json")
        w := httptest.NewRecorder()
        router.ServeHTTP(w, req)
        assert.Equal(t, http.StatusUnauthorized, w.Code, "attempt %d should be 401", i)
    }

    req := httptest.NewRequest(http.MethodPost, "/login", bytes.NewReader(body))
    req.Header.Set("Content-Type", "application/json")
    w := httptest.NewRecorder()
    router.ServeHTTP(w, req)
    assert.Equal(t, http.StatusLocked, w.Code, "4th attempt should be 423")
}
```

- [ ] **Step 2: 运行所�?handler 测试**

```bash
cd services/auth-service && go test ./handlers/ -v
```

期望：全�?PASS�?

- [ ] **Step 3: �?go vet �?gofmt**

```bash
cd services/auth-service && go vet ./... && gofmt -l . | Select-String "."
```

期望：vet 0 输出；gofmt 0 输出�?

- [ ] **Step 4: 提交**

```bash
git add services/auth-service/handlers/auth_test.go
git commit -m "test(auth-service): 登录锁定端到端集成测�?
```

---

### Task 9: main.go TrustedProxies

**Files:**
- Modify: `services/auth-service/main.go`

- [ ] **Step 1: 配置 trusted proxies**

�?`main.go` �?`server := &Server{...}` 之后、`server.setupRoutes()` 之前加：

```go
server.engine.SetTrustedProxies([]string{"0.0.0.0/0", "::/0"})
```

- [ ] **Step 2: 编译验证**

```bash
cd services/auth-service && go build ./...
```

期望：无错误�?

- [ ] **Step 3: 提交**

```bash
git add services/auth-service/main.go
git commit -m "chore(auth-service): 配置 trusted proxies 取得真实 IP"
```

---

## 验收清单

- [ ] `go vet ./...` 无输�?
- [ ] `gofmt -l .` 无输�?
- [ ] `go test ./handlers/ -v` 全部通过
- [ ] 配置缺失时默认值生�?
- [ ] 锁定后返�?423 �?retry_after
- [ ] 失败 6 次后（默认阈值）自动锁定
- [ ] 成功后计数器与锁 key 被清�?
- [ ] 账号不存在也走失败计�?
- [ ] LoginLog �?success/username/fail_reason 字段落库

## 风险

- 集成测试需要本�?Redis 在线（已�?`skipIfRedisUnavailable` 兜底�?
- 修改 `recordLog` 接口会引发编译连锁，集中�?Task 7 完成
- `c.ClientIP()` 在反代未�?trusted proxies 时拿到的是反�?IP
