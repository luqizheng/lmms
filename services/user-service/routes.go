package main

import (
	"context"
	"net/http"
	"strconv"
	"superpowers/user-service/handlers"
	"time"

	jwtctx "github.com/coder-micro-services/go-jwt-ctx"
	"github.com/gin-gonic/gin"
	swaggerFiles "github.com/swaggo/files"
	ginSwagger "github.com/swaggo/gin-swagger"
	"go.uber.org/zap"
)

func (s *Server) setupRoutes() {
	s.engine.GET("/swagger/*any", ginSwagger.WrapHandler(swaggerFiles.Handler))

	s.engine.GET("/health", s.healthHandler)

	// 内部端点不经 api-gateway，不挂 JWT 中间件，仅内网访问
	internal := s.engine.Group("/internal")
	{
		internal.GET("/users/:id/claims", handlers.GetUserClaims(s.db))
	}

	// 业务路由挂 JWT 中间件。principalBridge 把 Principal.UserID 写回 user_id，
	// 兼容旧 handler 内部 c.GetInt("user_id") 的读取方式。
	api := s.engine.Group("/api")
	api.Use(jwtctx.Middleware(), principalBridge())

	users := api.Group("/users")
	{
		users.GET("/me", handlers.GetCurrentUser(s.db))
		users.PUT("/me", handlers.UpdateCurrentUser(s.db))
		users.GET("", handlers.ListUsers(s.db))
		users.GET("/:id", handlers.GetUser(s.db))
		users.PATCH("/:id/extends", handlers.PatchUserExtends(s.db))
	}

	roles := api.Group("/roles")
	{
		roles.GET("", handlers.ListRoles(s.db))
		roles.POST("", handlers.CreateRole(s.db))
		roles.GET("/:id", handlers.GetRole(s.db))
		roles.PUT("/:id", handlers.UpdateRole(s.db))
		roles.DELETE("/:id", handlers.DeleteRole(s.db))
		roles.POST("/:id/permissions", handlers.AssignRolePermissions(s.db))
	}

	permissions := api.Group("/permissions")
	{
		permissions.GET("", handlers.ListPermissions(s.db))
	}

	userRoles := api.Group("/users/:id/roles")
	{
		userRoles.POST("", handlers.AssignUserRoles(s.db))
		userRoles.DELETE("", handlers.RemoveUserRoles(s.db))
	}
}

// principalBridge 把 JWT Principal.UserID（string）转成 int 写入 gin context["user_id"]，
// 让现有 handler 内的 c.GetInt("user_id") 仍能取到值。新 handler 应优先用 jwtctx.From(c)。
func principalBridge() gin.HandlerFunc {
	return func(c *gin.Context) {
		p := jwtctx.From(c)
		if p == nil {
			return
		}
		if uid, err := strconv.ParseUint(p.UserID, 10, 64); err == nil {
			c.Set("user_id", int(uid))
		}
	}
}

func (s *Server) healthHandler(c *gin.Context) {
	logger, _ := zap.NewProduction()
	defer logger.Sync()

	result := gin.H{
		"status":     "unknown",
		"timestamp":  time.Now().UTC().Format(time.RFC3339),
		"service":    "user-service",
		"components": gin.H{},
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	if err := s.db.Ping(ctx); err != nil {
		logger.Error("database connection failed", zap.Error(err))
		result["status"] = "degraded"
		result["components"] = gin.H{"database": "down"}
		c.JSON(http.StatusServiceUnavailable, result)
		return
	}

	result["status"] = "healthy"
	result["components"] = gin.H{"database": "up"}
	c.JSON(http.StatusOK, result)
}
