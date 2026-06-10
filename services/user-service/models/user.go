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
	ID        uint            `gorm:"primaryKey" json:"id"`
	Username  string          `gorm:"uniqueIndex;size:64;not null" json:"username"`
	Email     string          `gorm:"size:128" json:"email"`
	Phone     string          `gorm:"size:20" json:"phone"`
	Nickname  string          `gorm:"size:64" json:"nickname"`
	Status    int8            `gorm:"default:1" json:"status"`
	Roles     []Role          `gorm:"many2many:usr_user_role;" json:"roles"`
	Profile   UserProfile     `gorm:"foreignKey:UserID" json:"profile"`
	Extends   json.RawMessage `gorm:"type:text" json:"-"`
	CreatedAt time.Time       `json:"created_at"`
	UpdatedAt time.Time       `json:"updated_at"`
	DeletedAt gorm.DeletedAt  `gorm:"index" json:"-"`
}

func (User) TableName() string {
	return "usr_user"
}
