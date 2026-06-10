package handlers

import (
	"errors"
	"net/http"
	"strconv"

	"superpowers/user-service/models"
	"superpowers/user-service/store"

	"github.com/gin-gonic/gin"
	"gorm.io/gorm"
)

// GetUserClaims 返回 user-service 聚合的 JWT 业务 claims。
// 路径：GET /internal/users/:id/claims
// 仅内网调用（不经 api-gateway），供 token-server 在签 access token 前获取业务字段。
//
// Schema 由本函数决定，token-server 透传不改字段名。
// 当前返回：roles（来自 usr_user_role）、permissions（来自 usr_role_permission）。
// dept_id / is_dept_leader 等业务字段等 user-service 模型扩展后再加。
func GetUserClaims(db *store.DB) gin.HandlerFunc {
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

func buildUserClaims(db *store.DB, userID uint) (gin.H, error) {
	var user models.User
	if err := db.DB.
		Preload("Roles.Permissions").
		First(&user, userID).Error; err != nil {
		return nil, err
	}
	return buildUserClaimsFromRoles(user.Roles), nil
}

// buildUserClaimsFromRoles 纯函数：roles + permissions 聚合，permissions 去重。
func buildUserClaimsFromRoles(roles []models.Role) gin.H {
	outRoles := make([]string, 0, len(roles))
	outPerms := make([]string, 0)
	seen := map[string]struct{}{}
	for _, r := range roles {
		outRoles = append(outRoles, r.Code)
		for _, p := range r.Permissions {
			if _, ok := seen[p.Code]; ok {
				continue
			}
			seen[p.Code] = struct{}{}
			outPerms = append(outPerms, p.Code)
		}
	}
	return gin.H{
		"roles":       outRoles,
		"permissions": outPerms,
	}
}
