package handlers

import (
	"encoding/json"

	"superpowers/user-service/models"
)

// UserDTO 用户响应 DTO，扩展字段打平到顶层
type UserDTO struct {
	ID        uint               `json:"id"`
	Username  string             `json:"username"`
	Email     string             `json:"email"`
	Phone     string             `json:"phone"`
	Nickname  string             `json:"nickname"`
	Status    int8               `json:"status"`
	Roles     []string           `json:"roles"`
	Profile   models.UserProfile `json:"profile"`
	CreatedAt string             `json:"created_at"`
	UpdatedAt string             `json:"updated_at"`
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
