package handlers

import (
	"encoding/json"
	"net/http"
	"strconv"

	"superpowers/user-service/models"
	"superpowers/user-service/store"

	"github.com/gin-gonic/gin"
)

// GetCurrentUser 获取当前用户信息
// @Summary 获取当前用户信息
// @Description 获取当前登录用户的详细信息
// @Tags users
// @Accept json
// @Produce json
// @Success 200 {object} UserDTO
// @Failure 401 {object} map[string]string
// @Failure 404 {object} map[string]string
// @Router /api/users/me [get]
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

// UpdateCurrentUser 更新当前用户信息
// @Summary 更新当前用户信息
// @Description 更新当前登录用户的信息
// @Tags users
// @Accept json
// @Produce json
// @Param user body object true "用户信息"
// @Success 200 {object} UserDTO
// @Failure 400 {object} map[string]string
// @Failure 401 {object} map[string]string
// @Failure 404 {object} map[string]string
// @Failure 500 {object} map[string]string
// @Router /api/users/me [put]
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

// ListUsers 获取用户列表
// @Summary 获取用户列表
// @Description 分页获取用户列表
// @Tags users
// @Accept json
// @Produce json
// @Param page query int false "页码"
// @Param page_size query int false "每页数量"
// @Success 200 {object} map[string]interface{}
// @Failure 500 {object} map[string]string
// @Router /api/users [get]
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

// GetUser 获取用户详情
// @Summary 获取用户详情
// @Description 根据ID获取用户详细信息
// @Tags users
// @Accept json
// @Produce json
// @Param id path int true "用户ID"
// @Success 200 {object} UserDTO
// @Failure 400 {object} map[string]string
// @Failure 404 {object} map[string]string
// @Router /api/users/{id} [get]
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
