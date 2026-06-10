package main

import (
	"context"
	"net/http"
	"time"

	"superpowers/auth-service/handlers"

	jwtctx "github.com/coder-micro-services/go-jwt-ctx"
	"github.com/gin-gonic/gin"
	"go.uber.org/zap"
)

func (s *Server) setupRoutes() {
	s.engine.GET("/health", s.healthHandler)

	// 公开：register / login 不需要 JWT
	authPublic := s.engine.Group("/api/auth")
	{
		authPublic.POST("/register", handlers.Register(s.db))
		authPublic.POST("/login", handlers.Login(s.db, s.redis, s.cfg))
	}

	// 受保护：logout 等需要 JWT
	authProtected := s.engine.Group("/api/auth")
	authProtected.Use(jwtctx.Middleware())
	{
		authProtected.POST("/logout", handlers.Logout(s.redis, s.cfg))
	}
}

func (s *Server) healthHandler(c *gin.Context) {
	logger, _ := zap.NewProduction()
	defer logger.Sync()

	result := gin.H{
		"status":      "unknown",
		"timestamp":   time.Now().UTC().Format(time.RFC3339),
		"service":     "auth-service",
		"components":  gin.H{},
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

	if err := s.redis.Ping(ctx).Err(); err != nil {
		logger.Error("redis connection failed", zap.Error(err))
		result["status"] = "degraded"
		result["components"] = gin.H{"database": "up", "redis": "down"}
		c.JSON(http.StatusServiceUnavailable, result)
		return
	}

	result["status"] = "healthy"
	result["components"] = gin.H{"database": "up", "redis": "up"}
	c.JSON(http.StatusOK, result)
}
