package models

import (
	"time"

	"gorm.io/gorm"
)

// Role 角色
type Role struct {
	ID          uint           `gorm:"primaryKey" json:"id"`
	Name        string         `gorm:"uniqueIndex;size:64;not null" json:"name"`
	Code        string         `gorm:"uniqueIndex;size:64;not null" json:"code"`
	Description string         `gorm:"size:256" json:"description"`
	Status      int8           `gorm:"default:1" json:"status"`
	SortOrder   int            `gorm:"default:0" json:"sort_order"`
	Permissions []Permission   `gorm:"many2many:usr_role_permission;" json:"permissions"`
	Users       []User         `gorm:"many2many:usr_user_role;" json:"-"`
	CreatedAt   time.Time      `json:"created_at"`
	UpdatedAt   time.Time      `json:"updated_at"`
	DeletedAt   gorm.DeletedAt `gorm:"index" json:"-"`
}

func (Role) TableName() string {
	return "usr_role"
}

// UserRole 用户角色关联
type UserRole struct {
	UserID uint `gorm:"primaryKey" json:"user_id"`
	RoleID uint `gorm:"primaryKey" json:"role_id"`
}

func (UserRole) TableName() string {
	return "usr_user_role"
}
