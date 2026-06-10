package handlers

import (
	"net/http"
	"strconv"

	"superpowers/user-service/models"
	"superpowers/user-service/store"

	"github.com/gin-gonic/gin"
)

// ListRoles 获取角色列表
// @Summary 获取角色列表
// @Description 获取所有角色及其权限
// @Tags roles
// @Accept json
// @Produce json
// @Success 200 {array} models.Role
// @Failure 500 {object} map[string]string
// @Router /api/roles [get]
func ListRoles(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		var roles []models.Role
		if err := db.DB.Preload("Permissions").Find(&roles).Error; err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to list roles"})
			return
		}
		c.JSON(http.StatusOK, roles)
	}
}

// CreateRole 创建角色
// @Summary 创建角色
// @Description 创建新角色并关联权限
// @Tags roles
// @Accept json
// @Produce json
// @Param role body object true "角色信息"
// @Success 201 {object} models.Role
// @Failure 400 {object} map[string]string
// @Failure 409 {object} map[string]string
// @Router /api/roles [post]
func CreateRole(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		var req struct {
			Name        string `json:"name" binding:"required"`
			Code        string `json:"code" binding:"required"`
			Description string `json:"description"`
			Permissions []uint `json:"permissions"`
		}

		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
			return
		}

		role := models.Role{
			Name:        req.Name,
			Code:        req.Code,
			Description: req.Description,
			Status:      1,
		}

		if err := db.DB.Create(&role).Error; err != nil {
			c.JSON(http.StatusConflict, gin.H{"error": "role code already exists"})
			return
		}

		if len(req.Permissions) > 0 {
			var permissions []models.Permission
			db.DB.Find(&permissions, req.Permissions)
			db.DB.Model(&role).Association("Permissions").Append(permissions)
		}

		db.DB.Preload("Permissions").First(&role, role.ID)
		c.JSON(http.StatusCreated, role)
	}
}

// GetRole 获取角色详情
// @Summary 获取角色详情
// @Description 根据ID获取角色详细信息
// @Tags roles
// @Accept json
// @Produce json
// @Param id path int true "角色ID"
// @Success 200 {object} models.Role
// @Failure 400 {object} map[string]string
// @Failure 404 {object} map[string]string
// @Router /api/roles/{id} [get]
func GetRole(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		id, err := strconv.ParseUint(c.Param("id"), 10, 32)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid role id"})
			return
		}

		var role models.Role
		if err := db.DB.Preload("Permissions").First(&role, id).Error; err != nil {
			c.JSON(http.StatusNotFound, gin.H{"error": "role not found"})
			return
		}

		c.JSON(http.StatusOK, role)
	}
}

// UpdateRole 更新角色
// @Summary 更新角色
// @Description 更新角色信息
// @Tags roles
// @Accept json
// @Produce json
// @Param id path int true "角色ID"
// @Param role body object true "角色信息"
// @Success 200 {object} models.Role
// @Failure 400 {object} map[string]string
// @Failure 404 {object} map[string]string
// @Router /api/roles/{id} [put]
func UpdateRole(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		id, err := strconv.ParseUint(c.Param("id"), 10, 32)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid role id"})
			return
		}

		var req struct {
			Name        string `json:"name"`
			Description string `json:"description"`
			Status      int8   `json:"status"`
		}

		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
			return
		}

		var role models.Role
		if err := db.DB.First(&role, id).Error; err != nil {
			c.JSON(http.StatusNotFound, gin.H{"error": "role not found"})
			return
		}

		if req.Name != "" {
			role.Name = req.Name
		}
		if req.Description != "" {
			role.Description = req.Description
		}
		if req.Status != 0 {
			role.Status = req.Status
		}

		db.DB.Save(&role)
		db.DB.Preload("Permissions").First(&role, id)
		c.JSON(http.StatusOK, role)
	}
}

// DeleteRole 删除角色
// @Summary 删除角色
// @Description 根据ID删除角色
// @Tags roles
// @Accept json
// @Produce json
// @Param id path int true "角色ID"
// @Success 200 {object} map[string]string
// @Failure 400 {object} map[string]string
// @Failure 500 {object} map[string]string
// @Router /api/roles/{id} [delete]
func DeleteRole(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		id, err := strconv.ParseUint(c.Param("id"), 10, 32)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid role id"})
			return
		}

		if err := db.DB.Delete(&models.Role{}, id).Error; err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to delete role"})
			return
		}

		c.JSON(http.StatusOK, gin.H{"message": "role deleted"})
	}
}

// AssignRolePermissions 分配角色权限
// @Summary 分配角色权限
// @Description 为角色分配权限
// @Tags roles
// @Accept json
// @Produce json
// @Param id path int true "角色ID"
// @Param permissions body object true "权限ID列表"
// @Success 200 {object} models.Role
// @Failure 400 {object} map[string]string
// @Failure 404 {object} map[string]string
// @Router /api/roles/{id}/permissions [post]
func AssignRolePermissions(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		id, err := strconv.ParseUint(c.Param("id"), 10, 32)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid role id"})
			return
		}

		var req struct {
			Permissions []uint `json:"permissions"`
		}

		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
			return
		}

		var role models.Role
		if err := db.DB.First(&role, id).Error; err != nil {
			c.JSON(http.StatusNotFound, gin.H{"error": "role not found"})
			return
		}

		db.DB.Model(&role).Association("Permissions").Clear()

		if len(req.Permissions) > 0 {
			var permissions []models.Permission
			db.DB.Find(&permissions, req.Permissions)
			db.DB.Model(&role).Association("Permissions").Append(permissions)
		}

		db.DB.Preload("Permissions").First(&role, id)
		c.JSON(http.StatusOK, role)
	}
}

// ListPermissions 获取权限列表
// @Summary 获取权限列表
// @Description 获取所有权限
// @Tags permissions
// @Accept json
// @Produce json
// @Success 200 {array} models.Permission
// @Failure 500 {object} map[string]string
// @Router /api/permissions [get]
func ListPermissions(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		var permissions []models.Permission
		if err := db.DB.Find(&permissions).Error; err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to list permissions"})
			return
		}
		c.JSON(http.StatusOK, permissions)
	}
}

// AssignUserRoles 为用户分配角色
// @Summary 为用户分配角色
// @Description 为用户分配角色
// @Tags users
// @Accept json
// @Produce json
// @Param id path int true "用户ID"
// @Param roles body object true "角色ID列表"
// @Success 200 {object} models.User
// @Failure 400 {object} map[string]string
// @Failure 404 {object} map[string]string
// @Router /api/users/{id}/roles [post]
func AssignUserRoles(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		id, err := strconv.ParseUint(c.Param("id"), 10, 32)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid user id"})
			return
		}

		var req struct {
			Roles []uint `json:"roles"`
		}

		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
			return
		}

		var user models.User
		if err := db.DB.First(&user, id).Error; err != nil {
			c.JSON(http.StatusNotFound, gin.H{"error": "user not found"})
			return
		}

		db.DB.Model(&user).Association("Roles").Clear()

		if len(req.Roles) > 0 {
			var roles []models.Role
			db.DB.Find(&roles, req.Roles)
			db.DB.Model(&user).Association("Roles").Append(roles)
		}

		db.DB.Preload("Roles").Preload("Profile").First(&user, id)
		c.JSON(http.StatusOK, user)
	}
}

// RemoveUserRoles 移除用户角色
// @Summary 移除用户角色
// @Description 移除用户的角色
// @Tags users
// @Accept json
// @Produce json
// @Param id path int true "用户ID"
// @Param roles body object true "角色ID列表"
// @Success 200 {object} models.User
// @Failure 400 {object} map[string]string
// @Failure 404 {object} map[string]string
// @Router /api/users/{id}/roles [delete]
func RemoveUserRoles(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		id, err := strconv.ParseUint(c.Param("id"), 10, 32)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid user id"})
			return
		}

		var req struct {
			Roles []uint `json:"roles"`
		}

		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
			return
		}

		var user models.User
		if err := db.DB.First(&user, id).Error; err != nil {
			c.JSON(http.StatusNotFound, gin.H{"error": "user not found"})
			return
		}

		if len(req.Roles) > 0 {
			var roles []models.Role
			db.DB.Find(&roles, req.Roles)
			db.DB.Model(&user).Association("Roles").Delete(roles)
		}

		db.DB.Preload("Roles").Preload("Profile").First(&user, id)
		c.JSON(http.StatusOK, user)
	}
}
