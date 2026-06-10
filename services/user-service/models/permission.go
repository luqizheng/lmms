package models

import (
	"time"

	"gorm.io/gorm"
)

// Permission 权限
type Permission struct {
	ID          uint           `gorm:"primaryKey" json:"id"`
	Name        string         `gorm:"uniqueIndex;size:64;not null" json:"name"`
	Code        string         `gorm:"uniqueIndex;size:128;not null" json:"code"`
	Description string         `gorm:"size:256" json:"description"`
	Type        int8           `gorm:"default:1" json:"type"`
	ParentID    *uint          `json:"parent_id"`
	Status      int8           `gorm:"default:1" json:"status"`
	SortOrder   int            `gorm:"default:0" json:"sort_order"`
	CreatedAt   time.Time      `json:"created_at"`
	UpdatedAt   time.Time      `json:"updated_at"`
	DeletedAt   gorm.DeletedAt `gorm:"index" json:"-"`
}

func (Permission) TableName() string {
	return "usr_permission"
}

// RolePermission 角色权限关联
type RolePermission struct {
	RoleID       uint `gorm:"primaryKey" json:"role_id"`
	PermissionID uint `gorm:"primaryKey" json:"permission_id"`
}

func (RolePermission) TableName() string {
	return "usr_role_permission"
}
