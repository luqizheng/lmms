# User Service 动态扩展字段实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 User Service 增加动态扩展字段能力，扩展字段打平到 DTO 顶层输出

**Architecture:** 在 User 模型增加 Extends JSON 列存储扩展数据，Service 层负责将 extends 内容 merge 到 DTO 顶层

**Tech Stack:** Go, GORM, Gin, encoding/json

---

## 文件结构

| 文件 | 职责 |
|------|------|
| models/user.go | User 模型，增加 Extends 字段 |
| handlers/dto.go | 新建，定义 UserDTO 和 merge 逻辑 |
| handlers/user.go | 修改，使用 UserDTO 处理请求/响应 |
| handlers/user_test.go | 新建，测试 extends merge 逻辑 |

---

### Task 1: 增加 User.Extends 字段

**Files:**
- Modify: `services/user-service/models/user.go`

- [ ] **Step 1: 修改 User 模型，增加 Extends 字段**

```go
package models

import (
	"encoding/json"
	"time"

	"gorm.io/gorm"
)

// UserProfile 用户资料
type UserProfile struct {
	ID          uint           `gorm:"primaryKey" json:"id"`
	UserID      uint           `gorm:"uniqueIndex;not null" json:"user_id"`
	DisplayName string         `gorm:"size:128" json:"display_name"`
	Avatar      string         `gorm:"size:512" json:"avatar"`
	Bio         string         `gorm:"size:512" json:"bio"`
	CreatedAt   time.Time      `json:"created_at"`
	UpdatedAt   time.Time      `json:"updated_at"`
	DeletedAt   gorm.DeletedAt `gorm:"index" json:"-"`
}

func (UserProfile) TableName() string {
	return "usr_user_profile"
}

// User 用户
type User struct {
	ID        uint           `gorm:"primaryKey" json:"id"`
	Username  string         `gorm:"uniqueIndex;size:64;not null" json:"username"`
	Email     string         `gorm:"size:128" json:"email"`
	Phone     string         `gorm:"size:20" json:"phone"`
	Nickname  string         `gorm:"size:64" json:"nickname"`
	Status    int8           `gorm:"default:1" json:"status"`
	Roles     []Role         `gorm:"many2many:usr_user_role;" json:"roles"`
	Profile   UserProfile    `gorm:"foreignKey:UserID" json:"profile"`
	Extends   json.RawMessage `gorm:"type:text" json:"-"`
	CreatedAt time.Time      `json:"created_at"`
	UpdatedAt time.Time      `json:"updated_at"`
	DeletedAt gorm.DeletedAt `gorm:"index" json:"-"`
}

func (User) TableName() string {
	return "usr_user"
}
```

- [ ] **Step 2: 运行测试验证模型变更**

Run: `cd services/user-service && go build ./...`
Expected: 编译成功

---

### Task 2: 创建 UserDTO 和 merge 逻辑

**Files:**
- Create: `services/user-service/handlers/dto.go`

- [ ] **Step 1: 创建 dto.go，定义 UserDTO 和辅助函数**

```go
package handlers

import (
	"encoding/json"

	"superpowers/user-service/models"
)

// UserDTO 用户响应 DTO，扩展字段打平到顶层
type UserDTO struct {
	ID        uint                   `json:"id"`
	Username  string                 `json:"username"`
	Email     string                 `json:"email"`
	Phone     string                 `json:"phone"`
	Nickname  string                 `json:"nickname"`
	Status    int8                   `json:"status"`
	Roles     []string               `json:"roles"`
	Profile   models.UserProfile     `json:"profile"`
	CreatedAt string                 `json:"created_at"`
	UpdatedAt string                 `json:"updated_at"`
	// 扩展字段存储在 map 中，序列化时打平到顶层
	extends   map[string]interface{}
}

// 基础字段名集合，用于区分扩展字段
var baseFields = map[string]bool{
	"id": true, "username": true, "email": true, "phone": true,
	"nickname": true, "status": true, "roles": true, "profile": true,
	"created_at": true, "updated_at": true,
}

// ToDTO 将 User 转换为 UserDTO
func ToDTO(user *models.User) (*UserDTO, error) {
	dto := &UserDTO{
		ID:        user.ID,
		Username:  user.Username,
		Email:     user.Email,
		Phone:     user.Phone,
		Nickname:  user.Nickname,
		Status:    user.Status,
		Profile:   user.Profile,
		CreatedAt: user.CreatedAt.Format("2006-01-02T15:04:05Z07:00"),
		UpdatedAt: user.UpdatedAt.Format("2006-01-02T15:04:05Z07:00"),
	}

	// 提取角色名
	for _, role := range user.Roles {
		dto.Roles = append(dto.Roles, role.Name)
	}

	// 解析 extends
	if len(user.Extends) > 0 {
		var extends map[string]interface{}
		if err := json.Unmarshal(user.Extends, &extends); err != nil {
			return nil, err
		}
		dto.extends = extends
	}

	return dto, nil
}

// MarshalJSON 自定义 JSON 序列化，将 extends 打平到顶层
func (dto *UserDTO) MarshalJSON() ([]byte, error) {
	// 先序列化基础字段
	type Alias UserDTO
	base, err := json.Marshal(&struct {
		Roles []string `json:"roles"`
		*Alias
	}{
		Roles: dto.Roles,
		Alias: (*Alias)(dto),
	})
	if err != nil {
		return nil, err
	}

	if len(dto.extends) == 0 {
		return base, nil
	}

	// 合并 extends 到顶层
	var result map[string]interface{}
	if err := json.Unmarshal(base, &result); err != nil {
		return nil, err
	}

	for k, v := range dto.extends {
		if !baseFields[k] {
			result[k] = v
		}
	}

	return json.Marshal(result)
}

// ParseExtends 从请求中提取扩展字段
func ParseExtends(req map[string]interface{}) map[string]interface{} {
	extends := make(map[string]interface{})
	for k, v := range req {
		if !baseFields[k] {
			extends[k] = v
		}
	}
	return extends
}
```

- [ ] **Step 2: 验证编译**

Run: `cd services/user-service && go build ./...`
Expected: 编译成功

---

### Task 3: 更新 handlers 使用 UserDTO

**Files:**
- Modify: `services/user-service/handlers/user.go`

- [ ] **Step 1: 修改 GetCurrentUser 使用 UserDTO**

在 `GetCurrentUser` 函数中，将 `c.JSON(http.StatusOK, user)` 替换为：

```go
func GetCurrentUser(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		userID := c.GetInt("user_id")
		if userID == 0 {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "unauthorized"})
			return
		}

		var user models.User
		if err := db.DB.Preload("Roles").Preload("Profile").First(&user, userID).Error; err != nil {
			c.JSON(http.StatusNotFound, gin.H{"error": "user not found"})
			return
		}

		dto, err := ToDTO(&user)
		if err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to parse user extends"})
			return
		}
		c.JSON(http.StatusOK, dto)
	}
}
```

- [ ] **Step 2: 修改 UpdateCurrentUser 支持 extends**

```go
func UpdateCurrentUser(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		userID := c.GetInt("user_id")
		if userID == 0 {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "unauthorized"})
			return
		}

		var req map[string]interface{}
		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
			return
		}

		var user models.User
		if err := db.DB.First(&user, userID).Error; err != nil {
			c.JSON(http.StatusNotFound, gin.H{"error": "user not found"})
			return
		}

		// 更新基础字段
		if v, ok := req["nickname"].(string); ok {
			user.Nickname = v
		}
		if v, ok := req["email"].(string); ok {
			user.Email = v
		}
		if v, ok := req["phone"].(string); ok {
			user.Phone = v
		}

		// 更新 extends
		extends := ParseExtends(req)
		if len(extends) > 0 {
			extendsJSON, _ := json.Marshal(extends)
			user.Extends = extendsJSON
		}

		if err := db.DB.Save(&user).Error; err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to update user"})
			return
		}

		// 更新 profile
		var profile models.UserProfile
		if err := db.DB.Where("user_id = ?", userID).First(&profile).Error; err != nil {
			profile = models.UserProfile{UserID: uint(userID)}
		}

		if v, ok := req["display_name"].(string); ok {
			profile.DisplayName = v
		}
		if v, ok := req["bio"].(string); ok {
			profile.Bio = v
		}
		if v, ok := req["avatar"].(string); ok {
			profile.Avatar = v
		}

		db.DB.Save(&profile)

		db.DB.Preload("Roles").Preload("Profile").First(&user, userID)
		dto, _ := ToDTO(&user)
		c.JSON(http.StatusOK, dto)
	}
}
```

- [ ] **Step 3: 修改 GetUser 使用 UserDTO**

```go
func GetUser(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		id, err := strconv.ParseUint(c.Param("id"), 10, 32)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid user id"})
			return
		}

		var user models.User
		if err := db.DB.Preload("Roles").Preload("Profile").First(&user, id).Error; err != nil {
			c.JSON(http.StatusNotFound, gin.H{"error": "user not found"})
			return
		}

		dto, err := ToDTO(&user)
		if err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to parse user extends"})
			return
		}
		c.JSON(http.StatusOK, dto)
	}
}
```

- [ ] **Step 4: 修改 ListUsers 使用 UserDTO**

```go
func ListUsers(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		page, _ := strconv.Atoi(c.DefaultQuery("page", "1"))
		pageSize, _ := strconv.Atoi(c.DefaultQuery("page_size", "20"))

		if page < 1 {
			page = 1
		}
		if pageSize < 1 || pageSize > 100 {
			pageSize = 20
		}

		offset := (page - 1) * pageSize

		var users []models.User
		var total int64

		db.DB.Model(&models.User{}).Count(&total)
		if err := db.DB.Preload("Roles").Offset(offset).Limit(pageSize).Find(&users).Error; err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to list users"})
			return
		}

		dtos := make([]*UserDTO, 0, len(users))
		for i := range users {
			dto, err := ToDTO(&users[i])
			if err != nil {
				continue
			}
			dtos = append(dtos, dto)
		}

		c.JSON(http.StatusOK, gin.H{
			"data":      dtos,
			"total":     total,
			"page":      page,
			"page_size": pageSize,
		})
	}
}
```

- [ ] **Step 5: 添加必要的 import**

在 `handlers/user.go` 顶部添加：

```go
import (
	"encoding/json"
	"net/http"
	"strconv"

	"superpowers/user-service/models"
	"superpowers/user-service/store"

	"github.com/gin-gonic/gin"
)
```

- [ ] **Step 6: 验证编译**

Run: `cd services/user-service && go build ./...`
Expected: 编译成功

---

### Task 4: 增加 extends 部分更新接口

**Files:**
- Modify: `services/user-service/handlers/user.go`
- Modify: `services/user-service/routes.go`

- [ ] **Step 1: 添加 PatchUserExtends handler**

```go
// PatchUserExtends 部分更新用户扩展字段
// @Summary 部分更新用户扩展字段
// @Description 仅更新用户的扩展字段
// @Tags users
// @Accept json
// @Produce json
// @Param id path int true "用户ID"
// @Param extends body object true "扩展字段"
// @Success 200 {object} UserDTO
// @Failure 400 {object} map[string]string
// @Failure 404 {object} map[string]string
// @Failure 500 {object} map[string]string
// @Router /api/users/{id}/extends [patch]
func PatchUserExtends(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		id, err := strconv.ParseUint(c.Param("id"), 10, 32)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid user id"})
			return
		}

		var req map[string]interface{}
		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
			return
		}

		var user models.User
		if err := db.DB.First(&user, id).Error; err != nil {
			c.JSON(http.StatusNotFound, gin.H{"error": "user not found"})
			return
		}

		// 解析现有 extends
		var existing map[string]interface{}
		if len(user.Extends) > 0 {
			json.Unmarshal(user.Extends, &existing)
		}
		if existing == nil {
			existing = make(map[string]interface{})
		}

		// 合并新值
		for k, v := range req {
			existing[k] = v
		}

		extendsJSON, _ := json.Marshal(existing)
		user.Extends = extendsJSON

		if err := db.DB.Save(&user).Error; err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to update extends"})
			return
		}

		db.DB.Preload("Roles").Preload("Profile").First(&user, id)
		dto, _ := ToDTO(&user)
		c.JSON(http.StatusOK, dto)
	}
}
```

- [ ] **Step 2: 在 routes.go 注册路由**

读取 `routes.go`，在用户路由组中添加：

```go
users := api.Group("/users")
{
	users.GET("", ListUsers(db))
	users.GET("/:id", GetUser(db))
	users.PATCH("/:id/extends", PatchUserExtends(db))
}
```

- [ ] **Step 3: 验证编译**

Run: `cd services/user-service && go build ./...`
Expected: 编译成功

---

### Task 5: 编写测试

**Files:**
- Create: `services/user-service/handlers/dto_test.go`

- [ ] **Step 1: 创建 dto_test.go**

```go
package handlers

import (
	"encoding/json"
	"testing"

	"superpowers/user-service/models"
)

func TestToDTO(t *testing.T) {
	extends := `{"dept":"销售部","pos":"经理"}`
	user := &models.User{
		ID:       1,
		Username: "john",
		Email:    "john@example.com",
		Phone:    "13800138000",
		Nickname: "John",
		Status:   1,
		Extends:  json.RawMessage(extends),
		Roles:    []models.Role{{Name: "admin"}},
	}

	dto, err := ToDTO(user)
	if err != nil {
		t.Fatalf("ToDTO failed: %v", err)
	}

	if dto.Username != "john" {
		t.Errorf("expected username john, got %s", dto.Username)
	}

	if len(dto.extends) != 2 {
		t.Errorf("expected 2 extends, got %d", len(dto.extends))
	}
}

func TestUserDTO_MarshalJSON(t *testing.T) {
	dto := &UserDTO{
		ID:       1,
		Username: "john",
		Email:    "john@example.com",
		extends: map[string]interface{}{
			"dept": "销售部",
			"pos":  "经理",
		},
	}

	data, err := json.Marshal(dto)
	if err != nil {
		t.Fatalf("MarshalJSON failed: %v", err)
	}

	var result map[string]interface{}
	json.Unmarshal(data, &result)

	if result["username"] != "john" {
		t.Errorf("expected username john, got %v", result["username"])
	}

	if result["dept"] != "销售部" {
		t.Errorf("expected dept 销售部, got %v", result["dept"])
	}

	if result["pos"] != "经理" {
		t.Errorf("expected pos 经理, got %v", result["pos"])
	}
}

func TestParseExtends(t *testing.T) {
	req := map[string]interface{}{
		"username": "john",
		"email":    "john@example.com",
		"dept":     "销售部",
		"pos":      "经理",
	}

	extends := ParseExtends(req)

	if len(extends) != 2 {
		t.Errorf("expected 2 extends, got %d", len(extends))
	}

	if extends["dept"] != "销售部" {
		t.Errorf("expected dept 销售部, got %v", extends["dept"])
	}
}
```

- [ ] **Step 2: 运行测试**

Run: `cd services/user-service && go test ./handlers/... -v`
Expected: 所有测试通过

---

### Task 6: 提交变更

- [ ] **Step 1: 提交代码**

Run: `git add services/user-service/models/user.go services/user-service/handlers/dto.go services/user-service/handlers/user.go services/user-service/handlers/dto_test.go`
Run: `git commit -m "feat(user-service): add dynamic extends field support"`
Expected: 提交成功
