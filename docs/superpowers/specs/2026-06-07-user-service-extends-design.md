# User Service 动态扩展字段设计

## 概述

为 User Service 增加动态扩展字段能力，支持不同客户自定义用户属性，无需修改数据库结构。

## 需求

- 不同客户可增加不同的用户字段
- 支持存储、展示、查询
- 字段类型：字符串、数字
- 字段数量：每用户少于 10 个
- DTO 输出时扩展字段打平到顶层，与基础字段一致

## 技术方案

### 数据模型

**User 表** — 增加 `extends` 列

```go
type User struct {
    ID        uint           `gorm:"primaryKey" json:"id"`
    Username  string         `gorm:"uniqueIndex;size:64;not null" json:"username"`
    Email     string         `gorm:"size:128" json:"email"`
    Phone     string         `gorm:"size:20" json:"phone"`
    Nickname  string         `gorm:"size:64" json:"nickname"`
    Status    int8           `gorm:"default:1" json:"status"`
    Roles     []Role         `gorm:"many2many:usr_user_role;" json:"roles"`
    Profile   UserProfile    `gorm:"foreignKey:UserID" json:"profile"`
    Extends   json.RawMessage `gorm:"type:text" json:"-"` // 不直接序列化
    CreatedAt time.Time      `json:"created_at"`
    UpdatedAt time.Time      `json:"updated_at"`
    DeletedAt gorm.DeletedAt `gorm:"index" json:"-"`
}
```

`Extends` 存储 JSON 格式数据，例如：
```json
{
  "dept": "销售部",
  "pos": "经理"
}
```

### DTO 设计

**UserDTO** — 扩展字段打平到顶层

```go
type UserDTO struct {
    ID        uint     `json:"id"`
    Username  string   `json:"username"`
    Email     string   `json:"email"`
    Phone     string   `json:"phone"`
    Nickname  string   `json:"nickname"`
    Status    int8     `json:"status"`
    Roles     []string `json:"roles"`
    // 扩展字段动态添加
    Extends   map[string]interface{} `json:"-"` // 内部使用
}
```

**JSON 输出示例**：
```json
{
  "id": 1,
  "username": "john",
  "email": "john@example.com",
  "phone": "13800138000",
  "nickname": "John",
  "status": 1,
  "roles": ["admin"],
  "dept": "销售部",
  "pos": "经理"
}
```

### Service 层逻辑

1. **查询用户**：从数据库读取 User，解析 Extends JSON，merge 到 DTO 顶层
2. **创建/更新用户**：从 DTO 提取已知字段外的所有字段，存入 Extends JSON
3. **部分更新 extends**：读取现有 Extends，合并新值，保存

### API 接口

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | /users/{id} | 获取用户（扩展字段打平） |
| PUT | /users/{id} | 更新用户（可同时更新基础和扩展字段） |
| PATCH | /users/{id}/extends | 仅更新扩展字段 |

### 实现要点

1. GORM 自动迁移会增加 `extends` 列（TEXT 类型）
2. Extends 字段使用 `json.RawMessage` 避免双重编码
3. DTO 序列化时动态 merge extends 字段到顶层
4. 更新时校验 JSON 格式有效性
5. 基础字段名与扩展字段名不可冲突（基础字段优先）

## 文件变更

| 文件 | 变更 |
|------|------|
| models/user.go | 增加 Extends 字段 |
| handlers/user.go | 修改 DTO，增加 merge 逻辑 |
| store/db.go | 自动迁移（无需改动） |

## 测试要点

1. 创建用户时传入扩展字段，验证存储正确
2. 查询用户时验证扩展字段打平到顶层
3. 更新用户时验证扩展字段正确合并
4. 扩展字段与基础字段同名时，基础字段优先
