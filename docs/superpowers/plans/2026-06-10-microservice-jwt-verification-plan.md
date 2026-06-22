# 微服务间 JWT 验证（用户身份层）实现计�?

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 api-gateway 透传 JWT + 下游 Go 服务解码 user 身份 + 业务级授权所需的全部代码�?

**Architecture:**
- **api-gateway (Rust)**：验签后�?JWT claims 注入 `X-User-*` 冗余头，Authorization 原样透传
- **token-server (Rust)**：`issue_access_token` 改为接受 `extra_claims: serde_json::Value` 透传；access 路由在签发前 `reqwest` �?`user-service` 内部端点�?claims
- **user-service (Go)**：新�?`GET /internal/users/{id}/claims`，聚�?user + dept + roles + permissions �?claims JSON；引�?`libs/go-jwt-ctx` 中间�?
- **auth-service (Go)**：引�?`libs/go-jwt-ctx` 中间�?
- **libs/go-jwt-ctx (Go, 新建)**：JWT decode + 通用 `Principal` + gin middleware
- **libs/go-authz (Go, 新建)**：基于通用 Principal �?`CanRead/CanWrite` 辅助函数

**Tech Stack:** Rust 1.78+ / axum / jwt-simple / reqwest；Go 1.22+ / gin / golang-jwt/jwt v5

**Spec:** `docs/superpowers/specs/2026-06-10-microservice-jwt-verification-design.md`

**前置说明�?* Service-token（client_credentials 流程）不在本计划范围，spec �?5 节留接口待后续�?

---

## 范围检�?

设计文档�?1-4 节（用户身份�?+ 下游授权）→ 本计划覆盖�?
设计文档�?5 节（service-token）→ 留待后续独立 plan�?
设计文档�?10 节改动清单（�?service-token 相关项）�?本计划覆盖�?

---

## 文件结构

| 路径 | 状�?| 职责 |
|---|---|---|
| `libs/go-jwt-ctx/go.mod` | 新建 | Go module 声明 |
| `libs/go-jwt-ctx/principal.go` | 新建 | `Principal` 结构 + `Claims` 通用解码 |
| `libs/go-jwt-ctx/middleware.go` | 新建 | gin `AuthContextMiddleware` |
| `libs/go-jwt-ctx/middleware_test.go` | 新建 | 单元测试 |
| `libs/go-authz/go.mod` | 新建 | Go module 声明 |
| `libs/go-authz/authz.go` | 新建 | 通用 `HasPermission` / `HasRole` / `HasAnyRole` |
| `libs/go-authz/authz_test.go` | 新建 | 单元测试 |
| `services/user-service/handlers/claims.go` | 新建 | `GetUserClaims` handler |
| `services/user-service/handlers/claims_test.go` | 新建 | 端点测试 |
| `services/user-service/routes.go` | �?| 注册 `/internal/...` 路由�?|
| `services/user-service/main.go` | �?| 接入 `libs/go-jwt-ctx` 中间件（�?`/internal` �?`/health`�?|
| `services/user-service/go.mod` | �?| `require` libs/go-jwt-ctx |
| `services/auth-service/main.go` | �?| 接入 `libs/go-jwt-ctx` 中间件（�?`/api/auth/*` �?`/health`�?|
| `services/auth-service/go.mod` | �?| `require` libs/go-jwt-ctx |
| `services/token-server/src/issuer.rs` | �?| `issue_access_token` 接受 `extra_claims: serde_json::Value` |
| `services/token-server/src/routes/access.rs` | �?| �?access 前调 user-service �?claims |
| `services/token-server/src/routes/access_test.rs` | �?| 增加 mock user-service 的测�?|
| `services/api-gateway/src/middleware/mod.rs` | �?| 验签后注�?`X-User-*` �?|
| `services/api-gateway/src/middleware_test.rs` | 新建 | header 注入测试 |
| `tests/specs/user-service/claims-endpoint.yaml` | 新建 | E2E 规格 |

---

## Task 1: libs/go-jwt-ctx 模块初始�?

**Files:**
- Create: `libs/go-jwt-ctx/go.mod`
- Create: `libs/go-jwt-ctx/principal.go`

- [ ] **Step 1: 创建 go.mod**

```bash
New-Item -ItemType Directory -Path "libs/go-jwt-ctx" -Force
Set-Location "libs/go-jwt-ctx"
go mod init github.com/coder-micro-services/go-jwt-ctx
go get github.com/golang-jwt/jwt/v5@latest
go get github.com/gin-gonic/gin@latest
```

- [ ] **Step 2: �?Principal �?Claims 结构（先写骨架）**

`libs/go-jwt-ctx/principal.go`�?

```go
package jwtctx

import "encoding/json"

// Principal 是从 JWT 解析出的调用者身份，�?user-service 决定字段�?
// 通用字段放在顶层，业务字段放�?Custom 中按需读取�?
type Principal struct {
    UserID       string
    DepartmentID uint
    IsDeptLeader bool
    Roles        []string
    Permissions  map[string]struct{}
    Custom       map[string]any
}

// HasPermission 判断是否有指定权限点�?
func (p *Principal) HasPermission(perm string) bool {
    if p == nil {
        return false
    }
    _, ok := p.Permissions[perm]
    return ok
}

// HasRole 判断是否拥有指定角色�?
func (p *Principal) HasRole(role string) bool {
    if p == nil {
        return false
    }
    for _, r := range p.Roles {
        if r == role {
            return true
        }
    }
    return false
}

// Get 返回自定义字段，类型安全由调用方负责�?
func (p *Principal) Get(key string) (any, bool) {
    if p == nil {
        return nil, false
    }
    v, ok := p.Custom[key]
    return v, ok
}

// Claims �?JWT payload 的通用解码结果，Custom 存放 user-service 提供的所有业务字段�?
type Claims struct {
    Sub         string         `json:"sub"`
    Jti         string         `json:"jti"`
    Iss         string         `json:"iss"`
    Exp         int64          `json:"exp"`
    Iat         int64          `json:"iat"`
    ClientID    string         `json:"client_id"`
    Scope       string         `json:"scope"`
    Permissions []string       `json:"permissions"`
    DeptID      uint           `json:"dept_id"`
    IsDeptLead  bool           `json:"is_dept_leader"`
    Roles       []string       `json:"roles"`
    Custom      map[string]any `json:"-"`
}

// ToPrincipal �?Claims 转成 Principal，Permissions �?set 提升查询性能�?
func (c *Claims) ToPrincipal() *Principal {
    set := make(map[string]struct{}, len(c.Permissions))
    for _, p := range c.Permissions {
        set[p] = struct{}{}
    }
    return &Principal{
        UserID:       c.Sub,
        DepartmentID: c.DeptID,
        IsDeptLeader: c.IsDeptLead,
        Roles:        c.Roles,
        Permissions:  set,
        Custom:       c.Custom,
    }
}

// RawClaims �?json.RawMessage 抓取未在 struct 定义的字段，存进 Custom�?
type RawClaims struct {
    Claims
    Extra map[string]json.RawMessage `json:"-"`
}
```

- [ ] **Step 3: �?Decode 函数（解�?payload 不验签）**

`libs/go-jwt-ctx/decode.go`�?

```go
package jwtctx

import (
    "encoding/base64"
    "encoding/json"
    "errors"
    "strings"
    "time"
)

// ErrInvalidToken 标识 JWT 格式错误或解码失败�?
var ErrInvalidToken = errors.New("invalid_token")

// ErrTokenExpired 标识 JWT exp 已过�?
var ErrTokenExpired = errors.New("token_expired")

// Decode �?"Bearer xxx" 字符串中解析 claims�?*不做密码学验�?*�?
// 调用方需自行保证 token 来自可信来源（api-gateway）�?
func Decode(authHeader string) (*Claims, error) {
    if !strings.HasPrefix(authHeader, "Bearer ") {
        return nil, ErrInvalidToken
    }
    raw := strings.TrimPrefix(authHeader, "Bearer ")
    parts := strings.Split(raw, ".")
    if len(parts) != 3 {
        return nil, ErrInvalidToken
    }
    payload, err := base64.RawURLEncoding.DecodeString(parts[1])
    if err != nil {
        return nil, ErrInvalidToken
    }

    var rawClaims map[string]json.RawMessage
    if err := json.Unmarshal(payload, &rawClaims); err != nil {
        return nil, ErrInvalidToken
    }

    var c Claims
    if err := json.Unmarshal(payload, &c); err != nil {
        return nil, ErrInvalidToken
    }
    c.Custom = extractExtras(rawClaims, &c)
    return &c, nil
}

func extractExtras(raw map[string]json.RawMessage, c *Claims) map[string]any {
    known := map[string]struct{}{
        "sub": {}, "jti": {}, "iss": {}, "exp": {}, "iat": {},
        "client_id": {}, "scope": {}, "permissions": {},
        "dept_id": {}, "is_dept_leader": {}, "roles": {},
    }
    out := make(map[string]any)
    for k, v := range raw {
        if _, ok := known[k]; ok {
            continue
        }
        var anyVal any
        if err := json.Unmarshal(v, &anyVal); err == nil {
            out[k] = anyVal
        }
    }
    return out
}

// CheckExp 校验 exp，未过期返回 nil�?
func (c *Claims) CheckExp() error {
    if c.Exp == 0 {
        return nil
    }
    if time.Now().Unix() > c.Exp {
        return ErrTokenExpired
    }
    return nil
}
```

- [ ] **Step 4: 写中间件**

`libs/go-jwt-ctx/middleware.go`�?

```go
package jwtctx

import (
    "net/http"

    "github.com/gin-gonic/gin"
)

const principalKey = "user.principal"

// Middleware 返回 gin 中间件，�?Authorization 头解�?JWT 并注�?Principal �?context�?
// 解析失败返回 401，exp 过期返回 401，权限交�?handler 业务判断�?
func Middleware() gin.HandlerFunc {
    return func(c *gin.Context) {
        h := c.GetHeader("Authorization")
        if h == "" {
            c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"error": "missing_token"})
            return
        }
        claims, err := Decode(h)
        if err != nil {
            c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"error": "invalid_token"})
            return
        }
        if err := claims.CheckExp(); err != nil {
            c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"error": "token_expired"})
            return
        }
        c.Set(principalKey, claims.ToPrincipal())
        c.Next()
    }
}

// From �?gin context �?Principal，未设置时返�?nil�?
func From(c *gin.Context) *Principal {
    v, ok := c.Get(principalKey)
    if !ok {
        return nil
    }
    p, _ := v.(*Principal)
    return p
}
```

- [ ] **Step 5: 写单元测�?*

`libs/go-jwt-ctx/middleware_test.go`�?

```go
package jwtctx

import (
    "encoding/base64"
    "encoding/json"
    "net/http"
    "net/http/httptest"
    "testing"

    "github.com/gin-gonic/gin"
)

func makeToken(t *testing.T, claims map[string]any) string {
    t.Helper()
    header := base64.RawURLEncoding.EncodeToString([]byte(`{"alg":"RS256","typ":"JWT"}`))
    body, _ := json.Marshal(claims)
    payload := base64.RawURLEncoding.EncodeToString(body)
    sig := base64.RawURLEncoding.EncodeToString([]byte("fake-sig"))
    return header + "." + payload + "." + sig
}

func TestDecodeSuccess(t *testing.T) {
    tok := makeToken(t, map[string]any{
        "sub":            "1003",
        "exp":            9999999999,
        "dept_id":        42,
        "is_dept_leader": true,
        "roles":          []string{"sales", "dept_leader"},
        "permissions":    []string{"contract.read", "contract.write"},
        "region_id":      "cn-east",
    })
    c, err := Decode("Bearer " + tok)
    if err != nil {
        t.Fatalf("unexpected err: %v", err)
    }
    if c.Sub != "1003" || c.DeptID != 42 || !c.IsDeptLead {
        t.Fatalf("claim mismatch: %+v", c)
    }
    if c.Custom["region_id"] != "cn-east" {
        t.Fatalf("custom field missing: %+v", c.Custom)
    }
}

func TestDecodeMissingPrefix(t *testing.T) {
    if _, err := Decode("xxx"); err != ErrInvalidToken {
        t.Fatalf("expected ErrInvalidToken, got %v", err)
    }
}

func TestDecodeMalformed(t *testing.T) {
    if _, err := Decode("Bearer abc"); err != ErrInvalidToken {
        t.Fatalf("expected ErrInvalidToken, got %v", err)
    }
}

func TestCheckExp(t *testing.T) {
    expired := &Claims{Exp: 1}
    if err := expired.CheckExp(); err != ErrTokenExpired {
        t.Fatalf("expected expired, got %v", err)
    }
    ok := &Claims{Exp: 9999999999}
    if err := ok.CheckExp(); err != nil {
        t.Fatalf("unexpected err: %v", err)
    }
}

func TestMiddleware401OnMissing(t *testing.T) {
    gin.SetMode(gin.TestMode)
    r := gin.New()
    r.Use(Middleware())
    r.GET("/x", func(c *gin.Context) { c.Status(200) })

    w := httptest.NewRecorder()
    req := httptest.NewRequest("GET", "/x", nil)
    r.ServeHTTP(w, req)
    if w.Code != http.StatusUnauthorized {
        t.Fatalf("expected 401, got %d", w.Code)
    }
}

func TestMiddlewareInjectsPrincipal(t *testing.T) {
    gin.SetMode(gin.TestMode)
    r := gin.New()
    r.Use(Middleware())
    r.GET("/x", func(c *gin.Context) {
        p := From(c)
        if p == nil || p.UserID != "1003" {
            t.Fatalf("principal not injected: %+v", p)
        }
        c.Status(200)
    })
    tok := makeToken(t, map[string]any{"sub": "1003", "exp": 9999999999})

    w := httptest.NewRecorder()
    req := httptest.NewRequest("GET", "/x", nil)
    req.Header.Set("Authorization", "Bearer "+tok)
    r.ServeHTTP(w, req)
    if w.Code != 200 {
        t.Fatalf("expected 200, got %d", w.Code)
    }
}
```

- [ ] **Step 6: 跑测�?*

```bash
Set-Location "libs/go-jwt-ctx"
go test ./... -v
```

Expected: all PASS.

- [ ] **Step 7: 暂存（待用户确认 commit�?*

```bash
git add libs/go-jwt-ctx/
```

---

## Task 2: libs/go-authz 模块

**Files:**
- Create: `libs/go-authz/go.mod`
- Create: `libs/go-authz/authz.go`
- Create: `libs/go-authz/authz_test.go`

- [ ] **Step 1: 初始�?module**

```bash
New-Item -ItemType Directory -Path "libs/go-authz" -Force
Set-Location "libs/go-authz"
go mod init github.com/coder-micro-services/go-authz
go get github.com/coder-micro-services/go-jwt-ctx@latest
go get github.com/gin-gonic/gin@latest
```

> 实际�?go-jwt-ctx 还没发版，先�?replace 指令指向本地�?
```bash
go mod edit -replace github.com/coder-micro-services/go-jwt-ctx=../go-jwt-ctx
go mod tidy
```

- [ ] **Step 2: 写通用授权辅助**

`libs/go-authz/authz.go`�?

```go
package authz

import jwtctx "github.com/coder-micro-services/go-jwt-ctx"

// HasPermission 便捷方法，包一层便�?import 收敛�?
func HasPermission(p *jwtctx.Principal, perm string) bool {
    return p.HasPermission(perm)
}

// HasAnyPermission 任一权限命中即返�?true�?
func HasAnyPermission(p *jwtctx.Principal, perms ...string) bool {
    for _, perm := range perms {
        if p.HasPermission(perm) {
            return true
        }
    }
    return false
}

// HasAllPermissions 所有权限都需命中�?
func HasAllPermissions(p *jwtctx.Principal, perms ...string) bool {
    for _, perm := range perms {
        if !p.HasPermission(perm) {
            return false
        }
    }
    return true
}

// HasRole 便捷方法�?
func HasRole(p *jwtctx.Principal, role string) bool {
    return p.HasRole(role)
}

// HasAnyRole 任一角色命中即返�?true�?
func HasAnyRole(p *jwtctx.Principal, roles ...string) bool {
    for _, r := range roles {
        if p.HasRole(r) {
            return true
        }
    }
    return false
}

// RequirePermission 返回 gin abort 中间件，未命中返�?403�?
// 仅做粗粒度拦截，业务级判断仍�?handler 内部完成�?
import "github.com/gin-gonic/gin"
import "net/http"

func RequirePermission(perm string) gin.HandlerFunc {
    return func(c *gin.Context) {
        p := jwtctx.From(c)
        if p == nil || !p.HasPermission(perm) {
            c.AbortWithStatusJSON(http.StatusForbidden, gin.H{"error": "forbidden"})
            return
        }
        c.Next()
    }
}
```

> 注意：上�?import �?package 声明顺序要按 Go 规范调整。最终文件结构：

```go
package authz

import (
    "net/http"

    jwtctx "github.com/coder-micro-services/go-jwt-ctx"
    "github.com/gin-gonic/gin"
)

// ... 所有函�?
```

- [ ] **Step 3: 写单元测�?*

`libs/go-authz/authz_test.go`�?

```go
package authz

import (
    "net/http"
    "net/http/httptest"
    "testing"

    jwtctx "github.com/coder-micro-services/go-jwt-ctx"
    "github.com/gin-gonic/gin"
)

func mkPrincipal() *jwtctx.Principal {
    return &jwtctx.Principal{
        UserID: "1",
        Roles:  []string{"sales"},
        Permissions: map[string]struct{}{
            "contract.read": {},
        },
    }
}

func TestHasPermission(t *testing.T) {
    p := mkPrincipal()
    if !HasPermission(p, "contract.read") {
        t.Fatal("expected true")
    }
    if HasPermission(p, "contract.write") {
        t.Fatal("expected false")
    }
    if HasPermission(nil, "contract.read") {
        t.Fatal("nil principal should return false")
    }
}

func TestHasAnyPermission(t *testing.T) {
    p := mkPrincipal()
    if !HasAnyPermission(p, "contract.read", "contract.write") {
        t.Fatal("expected true (read hit)")
    }
    if HasAnyPermission(p, "contract.write", "user.delete") {
        t.Fatal("expected false")
    }
}

func TestHasAllPermissions(t *testing.T) {
    p := mkPrincipal()
    if HasAllPermissions(p, "contract.read", "contract.write") {
        t.Fatal("expected false (write missing)")
    }
}

func TestHasRole(t *testing.T) {
    p := mkPrincipal()
    if !HasRole(p, "sales") {
        t.Fatal("expected true")
    }
    if HasRole(p, "admin") {
        t.Fatal("expected false")
    }
}

func TestRequirePermission(t *testing.T) {
    gin.SetMode(gin.TestMode)
    r := gin.New()
    r.GET("/x", RequirePermission("contract.read"), func(c *gin.Context) { c.Status(200) })

    // 注入 principal
    r.GET("/y", func(c *gin.Context) {
        c.Set("user.principal", mkPrincipal())
        c.Next()
    }, RequirePermission("contract.read"), func(c *gin.Context) { c.Status(200) })

    w1 := httptest.NewRecorder()
    r.ServeHTTP(w1, httptest.NewRequest("GET", "/x", nil))
    if w1.Code != http.StatusForbidden {
        t.Fatalf("no principal: expected 403, got %d", w1.Code)
    }

    w2 := httptest.NewRecorder()
    r.ServeHTTP(w2, httptest.NewRequest("GET", "/y", nil))
    if w2.Code != http.StatusOK {
        t.Fatalf("with principal: expected 200, got %d", w2.Code)
    }
}
```

- [ ] **Step 4: 跑测�?*

```bash
Set-Location "libs/go-authz"
go test ./... -v
```

Expected: all PASS.

- [ ] **Step 5: 暂存**

```bash
git add libs/go-authz/
```

---

## Task 3: user-service 内部 claims 端点

**Files:**
- Create: `services/user-service/handlers/claims.go`
- Create: `services/user-service/handlers/claims_test.go`
- Modify: `services/user-service/routes.go`

- [ ] **Step 1: �?handler**

`services/user-service/handlers/claims.go`�?

```go
package handlers

import (
    "errors"
    "net/http"
    "strconv"

    "github.com/gin-gonic/gin"
    "gorm.io/gorm"
)

// GetUserClaims 返回 user-service 聚合�?JWT 业务 claims�?
// 路径：GET /internal/users/:id/claims
// 仅内网调用（不经 api-gateway），�?token-server 在签 access token 前获取业务字段�?
func GetUserClaims(db *gorm.DB) gin.HandlerFunc {
    return func(c *gin.Context) {
        id, err := strconv.ParseUint(c.Param("id"), 10, 64)
        if err != nil {
            c.JSON(http.StatusBadRequest, gin.H{"error": "invalid_id"})
            return
        }
        claims, err := buildUserClaims(db, uint(id))
        if err != nil {
            if errors.Is(err, gorm.ErrRecordNotFound) {
                c.JSON(http.StatusNotFound, gin.H{"error": "not_found"})
                return
            }
            c.JSON(http.StatusInternalServerError, gin.H{"error": "internal"})
            return
        }
        c.JSON(http.StatusOK, claims)
    }
}

// buildUserClaims 聚合 user + dept + roles + permissions �?claims 字典�?
// 字段 schema 由本函数决定，token-server 透传不改字段名�?
func buildUserClaims(db *gorm.DB, userID uint) (gin.H, error) {
    var u User
    if err := db.Preload("Roles.Permissions").First(&u, userID).Error; err != nil {
        return nil, err
    }
    perms := make([]string, 0)
    roles := make([]string, 0)
    seenPerm := map[string]struct{}{}
    for _, r := range u.Roles {
        roles = append(roles, r.Name)
        for _, p := range r.Permissions {
            if _, ok := seenPerm[p.Code]; ok {
                continue
            }
            seenPerm[p.Code] = struct{}{}
            perms = append(perms, p.Code)
        }
    }
    return gin.H{
        "dept_id":        u.DepartmentID,
        "is_dept_leader": u.IsDeptLeader,
        "roles":          roles,
        "permissions":    perms,
    }, nil
}
```

> 依赖类型 `User` 已存在于 `services/user-service/models/user.go`�?
> 包含 `Roles []Role` 关联。`Role.Permissions` �?`models/role.go`�?
> �?`User.DepartmentID` / `IsDeptLeader` 字段名不同，按实际表名调整�?

- [ ] **Step 2: 写测试（用内�?sqlite �?docker mysql�?*

`services/user-service/handlers/claims_test.go`�?

```go
package handlers

import (
    "encoding/json"
    "net/http"
    "net/http/httptest"
    "testing"

    "github.com/gin-gonic/gin"
    "gorm.io/driver/sqlite"
    "gorm.io/gorm"
    "superpowers/user-service/models"
)

func setupTestDB(t *testing.T) *gorm.DB {
    db, err := gorm.Open(sqlite.Open(":memory:"), &gorm.Config{})
    if err != nil {
        t.Fatal(err)
    }
    if err := db.AutoMigrate(&models.User{}, &models.Role{}, &models.Permission{}); err != nil {
        t.Fatal(err)
    }
    return db
}

func TestGetUserClaimsNotFound(t *testing.T) {
    db := setupTestDB(t)
    gin.SetMode(gin.TestMode)
    r := gin.New()
    r.GET("/internal/users/:id/claims", GetUserClaims(db))

    w := httptest.NewRecorder()
    req := httptest.NewRequest("GET", "/internal/users/9999/claims", nil)
    r.ServeHTTP(w, req)
    if w.Code != http.StatusNotFound {
        t.Fatalf("expected 404, got %d", w.Code)
    }
}

func TestGetUserClaimsAggregate(t *testing.T) {
    db := setupTestDB(t)
    // 插入 user + role + permission
    db.Create(&models.Permission{Code: "contract.read"})
    db.Create(&models.Permission{Code: "contract.write"})
    db.Create(&models.Role{Name: "sales"})
    db.Exec("INSERT INTO role_permissions (role_id, permission_id) VALUES (1, 1), (1, 2)")
    db.Create(&models.User{DepartmentID: 42, IsDeptLeader: true})
    db.Exec("INSERT INTO user_roles (user_id, role_id) VALUES (1, 1)")

    gin.SetMode(gin.TestMode)
    r := gin.New()
    r.GET("/internal/users/:id/claims", GetUserClaims(db))

    w := httptest.NewRecorder()
    req := httptest.NewRequest("GET", "/internal/users/1/claims", nil)
    r.ServeHTTP(w, req)
    if w.Code != http.StatusOK {
        t.Fatalf("expected 200, got %d body=%s", w.Code, w.Body.String())
    }
    var body map[string]any
    json.Unmarshal(w.Body.Bytes(), &body)
    if body["dept_id"].(float64) != 42 {
        t.Fatalf("dept_id mismatch: %+v", body)
    }
    if body["is_dept_leader"] != true {
        t.Fatalf("is_dept_leader mismatch: %+v", body)
    }
}
```

- [ ] **Step 3: 注册路由**

`services/user-service/routes.go` �?`setupRoutes` 内新增：

```go
internal := s.engine.Group("/internal")
{
    internal.GET("/users/:id/claims", handlers.GetUserClaims(s.db))
}
```

- [ ] **Step 4: 跑测�?*

```bash
Set-Location "services/user-service"
go test ./handlers/... -run TestGetUserClaims -v
```

Expected: all PASS.

- [ ] **Step 5: 暂存**

```bash
git add services/user-service/handlers/claims.go services/user-service/handlers/claims_test.go services/user-service/routes.go
```

---

## Task 4: user-service 接入 JWT 中间�?

**Files:**
- Modify: `services/user-service/main.go`
- Modify: `services/user-service/go.mod`

- [ ] **Step 1: 加依�?*

```bash
Set-Location "services/user-service"
go get github.com/coder-micro-services/go-jwt-ctx@latest
go mod edit -replace github.com/coder-micro-services/go-jwt-ctx=../../libs/go-jwt-ctx
go mod tidy
```

- [ ] **Step 2: 接入中间件（白名�?/internal �?/health�?*

`services/user-service/main.go` 中路由分组改为：

```go
func (s *Server) setupRoutes() {
    s.engine.GET("/health", s.healthHandler)

    // 内部端点不经 api-gateway，不�?JWT 中间�?
    internal := s.engine.Group("/internal")
    {
        internal.GET("/users/:id/claims", handlers.GetUserClaims(s.db))
    }

    // 业务路由�?JWT 中间�?
    api := s.engine.Group("/api")
    api.Use(jwtctx.Middleware())
    {
        users := api.Group("/users")
        users.GET("/me", handlers.GetCurrentUser(s.db))
        users.PUT("/me", handlers.UpdateCurrentUser(s.db))
        users.GET("", handlers.ListUsers(s.db))
        users.GET("/:id", handlers.GetUser(s.db))
        users.PATCH("/:id/extends", handlers.PatchUserExtends(s.db))

        roles := api.Group("/roles")
        roles.GET("", handlers.ListRoles(s.db))
        // ... 全部 roles 路由

        permissions := api.Group("/permissions")
        permissions.GET("", handlers.ListPermissions(s.db))

        userRoles := api.Group("/users/:id/roles")
        userRoles.POST("", handlers.AssignUserRoles(s.db))
        userRoles.DELETE("", handlers.RemoveUserRoles(s.db))
    }
}
```

- [ ] **Step 3: 编译验证**

```bash
Set-Location "services/user-service"
go build ./...
```

Expected: 0 errors.

- [ ] **Step 4: 暂存**

```bash
git add services/user-service/main.go services/user-service/go.mod services/user-service/go.sum
```

---

## Task 5: auth-service 接入 JWT 中间�?

**Files:**
- Modify: `services/auth-service/main.go`
- Modify: `services/auth-service/go.mod`

- [ ] **Step 1: 加依�?*

```bash
Set-Location "services/auth-service"
go get github.com/coder-micro-services/go-jwt-ctx@latest
go mod edit -replace github.com/coder-micro-services/go-jwt-ctx=../../libs/go-jwt-ctx
go mod tidy
```

- [ ] **Step 2: 接入中间�?*

`services/auth-service/main.go` 中：

```go
func (s *Server) setupRoutes() {
    s.engine.GET("/health", s.healthHandler)

    // /api/auth/login �?/register 不需�?JWT（登录用�?
    authPublic := s.engine.Group("/api/auth")
    {
        authPublic.POST("/register", handlers.Register(s.db))
        authPublic.POST("/login", handlers.Login(s.db, s.redis, s.cfg))
    }

    // /api/auth/logout 需�?JWT
    authProtected := s.engine.Group("/api/auth")
    authProtected.Use(jwtctx.Middleware())
    {
        authProtected.POST("/logout", handlers.Logout(s.redis, s.cfg))
    }
}
```

- [ ] **Step 3: 编译验证**

```bash
Set-Location "services/auth-service"
go build ./...
```

Expected: 0 errors.

- [ ] **Step 4: 暂存**

```bash
git add services/auth-service/main.go services/auth-service/go.mod services/auth-service/go.sum
```

---

## Task 6: token-server 改�?issuer 接受 extra_claims

**Files:**
- Modify: `services/token-server/src/issuer.rs`
- Modify: `services/token-server/src/routes/access.rs`
- Modify: `services/token-server/Cargo.toml`（如需 reqwest�?

- [ ] **Step 1: �?reqwest 依赖**

`services/token-server/Cargo.toml` �?`[dependencies]` 段加�?

```toml
reqwest = { version = "0.12", features = ["json"] }
```

（若已存在跳过。）

- [ ] **Step 2: �?issuer.rs**

替换 `issue_access_token` 签名为：

```rust
pub fn issue_access_token(
    &self,
    sub: &str,
    client_id: &str,
    extra_claims: serde_json::Value,
    ttl_sec: i64,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
    claims.subject = Some(sub.to_string());
    claims.jwt_id = Some(Uuid::new_v4().to_string());
    claims.issuer = Some(self.issuer.clone());
    claims.expires_at = Some(Self::now() + Duration::from_secs(ttl_sec as u64));

    let mut custom = HashMap::new();
    custom.insert("scope".to_string(), serde_json::Value::String("access".to_string()));
    custom.insert("client_id".to_string(), serde_json::Value::String(client_id.to_string()));
    // 合并 user-service 提供的业�?claims，user-service 决定 schema
    if let Some(obj) = extra_claims.as_object() {
        for (k, v) in obj {
            custom.insert(k.clone(), v.clone());
        }
    }
    claims.custom = custom;

    let token = self.key_pair.sign(claims)?;
    Ok(token)
}
```

> 同步移除旧的 `permissions` 参数调用（现在由 extra_claims 透传）�?

- [ ] **Step 3: �?access.rs：先�?user-service �?claims 再签�?*

`access.rs` 关键改动伪代码（保留原有 code 兑换逻辑）：

```rust
async fn handle_access(
    State(state): State<AppState>,
    Json(req): Json<AccessRequest>,
) -> Result<Json<AccessResponse>, (StatusCode, String)> {
    // 1. �?code �?user_id（保留旧逻辑�?
    let user_id = state.redis.get_code(&req.code).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::BAD_REQUEST, "invalid_code".to_string()))?;

    // 2. �?user-service 拿业�?claims
    let url = format!("{}/internal/users/{}/claims", state.config.user_service_url, user_id);
    let claims: serde_json::Value = state.http_client.get(&url).send().await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, format!("user_service_unreachable: {e}")))?
        .json().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("user_service_bad_response: {e}")))?;

    // 3. 校验 client
    state.clients.verify(&req.client_id, &req.client_secret)
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    // 4. 签发 access token
    let access = state.issuer.issue_access_token(&user_id, &req.client_id, claims, state.config.access_ttl_sec)?;

    // 5. �?refresh（不变）
    // ...

    Ok(Json(AccessResponse { access_token: access, refresh_token: refresh, ... }))
}
```

- [ ] **Step 4: �?user-service URL**

`services/token-server/src/config.rs` 增加字段（具体配置名按现有约定）�?

```rust
pub struct Config {
    // ...
    pub user_service_url: String,  // 例如 "http://user-service:8080"
}
```

`docker-example/config/token-server/token-server.toml` 加：

```toml
[upstream]
user_service_url = "http://user-service:8080"
```

- [ ] **Step 5: 跑测�?*

```bash
Set-Location "services/token-server"
cargo test
```

Expected: 现有测试 + 改造后用例 PASS�?

- [ ] **Step 6: 暂存**

```bash
git add services/token-server/
```

---

## Task 7: api-gateway 注入 X-User-* �?

**Files:**
- Modify: `services/api-gateway/src/middleware/mod.rs`
- Create: `services/api-gateway/src/middleware_test.rs`（追加测试到既有测试文件也行�?

- [ ] **Step 1: �?middleware/mod.rs 注入 header**

�?JWT 验签通过后（找到 `claims` 已被解析的位置），追加：

```rust
fn inject_user_headers(headers: &mut HeaderMap, claims: &JwtClaims) {
    headers.insert("X-User-Id", HeaderValue::from_str(&claims.sub).unwrap());
    if let Some(v) = claims.custom.get("dept_id") {
        if let Some(n) = v.as_u64() {
            headers.insert("X-User-Dept", HeaderValue::from(n));
        }
    }
    if let Some(v) = claims.custom.get("is_dept_leader") {
        if let Some(b) = v.as_bool() {
            headers.insert("X-User-Dept-Leader", HeaderValue::from_static(if b { "true" } else { "false" }));
        }
    }
    if let Some(v) = claims.custom.get("roles") {
        if let Some(arr) = v.as_array() {
            let joined = arr.iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(",");
            if let Ok(hv) = HeaderValue::from_str(&joined) {
                headers.insert("X-User-Roles", hv);
            }
        }
    }
    if let Some(v) = claims.custom.get("permissions") {
        if let Some(arr) = v.as_array() {
            let joined = arr.iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(",");
            if let Ok(hv) = HeaderValue::from_str(&joined) {
                headers.insert("X-User-Perms", hv);
            }
        }
    }
    if let Some(v) = claims.custom.get("client_id") {
        if let Some(s) = v.as_str() {
            if let Ok(hv) = HeaderValue::from_str(s) {
                headers.insert("X-User-Client", hv);
            }
        }
    }
}
```

调用点（验签成功后转发前）：

```rust
inject_user_headers(&mut proxied_headers, &claims);
```

- [ ] **Step 2: 写测�?*

`services/api-gateway/src/middleware_test.rs`（新建或追加�?`jwt_test.rs`）：

```rust
#[test]
fn inject_user_headers_reads_custom_claims() {
    let mut claims = JwtClaims::default();
    claims.sub = "1003".to_string();
    claims.custom.insert("dept_id".into(), json!(42));
    claims.custom.insert("is_dept_leader".into(), json!(true));
    claims.custom.insert("roles".into(), json!(["sales", "dept_leader"]));
    claims.custom.insert("permissions".into(), json!(["contract.read", "contract.write"]));
    claims.custom.insert("client_id".into(), json!("web-spa"));

    let mut headers = HeaderMap::new();
    super::inject_user_headers(&mut headers, &claims);

    assert_eq!(headers.get("X-User-Id").unwrap(), "1003");
    assert_eq!(headers.get("X-User-Dept").unwrap(), "42");
    assert_eq!(headers.get("X-User-Dept-Leader").unwrap(), "true");
    assert_eq!(headers.get("X-User-Roles").unwrap(), "sales,dept_leader");
    assert_eq!(headers.get("X-User-Perms").unwrap(), "contract.read,contract.write");
    assert_eq!(headers.get("X-User-Client").unwrap(), "web-spa");
}
```

- [ ] **Step 3: 跑测�?*

```bash
Set-Location "services/api-gateway"
cargo test middleware
```

Expected: PASS.

- [ ] **Step 4: 暂存**

```bash
git add services/api-gateway/
```

---

## Task 8: E2E 规格与运行验�?

**Files:**
- Create: `tests/specs/user-service/claims-endpoint.yaml`
- Create: `tests/specs/microservice-jwt-verification.yaml`

- [ ] **Step 1: �?claims endpoint 规格**

`tests/specs/user-service/claims-endpoint.yaml`�?

```yaml
name: user-service-claims-endpoint
description: |
  验证 GET /internal/users/{id}/claims 返回 user + dept + roles + permissions 聚合�?
  端点仅内网调用，不经 api-gateway，不�?JWT 中间件�?
scenarios:
  - name: 正常用户返回完整 claims
    steps:
      - seed:
          user: { id: 1, dept_id: 42, is_dept_leader: true }
          role: { name: sales }
          permission: { code: contract.read }
          bindings: { user_role: { user_id: 1, role_id: 1 }, role_permission: { role_id: 1, permission_id: 1 } }
      - request: { method: GET, path: /internal/users/1/claims }
      - expect: { status: 200, json: { dept_id: 42, is_dept_leader: true, roles: [sales], permissions: [contract.read] } }
  - name: 不存在的用户返回 404
    steps:
      - request: { method: GET, path: /internal/users/9999/claims }
      - expect: { status: 404, json: { error: not_found } }
```

- [ ] **Step 2: �?JWT 验证流程规格**

`tests/specs/microservice-jwt-verification.yaml`�?

```yaml
name: microservice-jwt-verification
description: |
  验证：登�?�?token-server �?user-service �?claims �?返回 JWT 含业务字�?�?
  api-gateway 注入 X-User-* �?�?下游 service 解析 Authorization �?Principal�?
scenarios:
  - name: 完整登录 + 业务访问链路
    steps:
      - request: { method: POST, path: /api/auth/login, body: { username: alice, password: x } }
      - expect: { status: 200, json: { code: "{{code}}" } }
      - request:
          method: POST
          path: /api/tokens/access
          body: { code: "{{code}}", grant_type: authorization_code, client_id: web-spa, client_secret: x }
      - expect:
          status: 200
          json:
            access_token: "{{access}}"
      - request:
          method: GET
          path: /api/users/me
          headers: { Authorization: "Bearer {{access}}" }
      - expect:
          status: 200
          # X-User-* 头由 api-gateway 注入
          headers_in_response_chain: { X-User-Id: "1", X-User-Dept: "42" }
```

- [ ] **Step 3: �?test-orchestrator �?*

```bash
Set-Location "d:\projects\coder-micro-services"
.\scripts\test-orchestrator.ps1 loop
```

Expected: 全部通过 / 现有 fail 数量不变�?

- [ ] **Step 4: 暂存**

```bash
git add tests/specs/
```

---

## 自检

- [x] Spec �?3 节（用户身份层）�?Task 1, 4, 5, 7
- [x] Spec �?4 节（下游业务授权）→ Task 2
- [x] Spec �?6.1 节（claims schema �?user-service 决定）→ Task 3, 6
- [x] Spec �?7 节（错误处理）→ Task 1 中间件已实现
- [x] Spec �?5 节（service-token）→ 明确排除（后续独�?plan�?
- [x] �?TBD / TODO 占位
- [x] 类型一致：`Principal.UserID` / `Claims.Sub` / `JwtClaims.sub` 三处语义统一

---

## 执行选项

1. **Subagent-Driven (推荐)** �?每个 task 起新 subagent，两阶段评审
2. **Inline Execution** �?在本会话中分批执行，�?checkpoint 复审

选哪个？
