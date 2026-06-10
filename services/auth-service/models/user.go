package models

import (
	"time"

	"gorm.io/gorm"
)

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

func (User) TableName() string {
	return "usr_user"
}

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
