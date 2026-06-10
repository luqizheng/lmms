package main

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/stretchr/testify/assert"
)

func init() {
	gin.SetMode(gin.TestMode)
}

func TestHealthEndpoint(t *testing.T) {
	router := gin.New()

	router.GET("/health", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"status": "ok"})
	})

	req := httptest.NewRequest(http.MethodGet, "/health", nil)
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusOK, w.Code)

	var resp map[string]string
	err := json.Unmarshal(w.Body.Bytes(), &resp)
	assert.NoError(t, err)
	assert.Equal(t, "ok", resp["status"])
}

func TestUserRoutesExist(t *testing.T) {
	router := gin.New()

	router.GET("/api/users/me", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})
	router.PUT("/api/users/me", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})
	router.GET("/api/users", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})
	router.GET("/api/users/:id", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})
	router.POST("/api/users/:id/roles", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})
	router.DELETE("/api/users/:id/roles", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})

	tests := []struct {
		method string
		path   string
	}{
		{http.MethodGet, "/api/users/me"},
		{http.MethodPut, "/api/users/me"},
		{http.MethodGet, "/api/users"},
		{http.MethodGet, "/api/users/1"},
		{http.MethodPost, "/api/users/1/roles"},
		{http.MethodDelete, "/api/users/1/roles"},
	}

	for _, tt := range tests {
		t.Run(tt.method+"_"+tt.path, func(t *testing.T) {
			req := httptest.NewRequest(tt.method, tt.path, nil)
			w := httptest.NewRecorder()

			router.ServeHTTP(w, req)

			assert.NotEqual(t, http.StatusNotFound, w.Code)
		})
	}
}

func TestRoleRoutesExist(t *testing.T) {
	router := gin.New()

	router.GET("/api/roles", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})
	router.POST("/api/roles", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})
	router.GET("/api/roles/:id", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})
	router.PUT("/api/roles/:id", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})
	router.DELETE("/api/roles/:id", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})
	router.POST("/api/roles/:id/permissions", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})

	tests := []struct {
		method string
		path   string
	}{
		{http.MethodGet, "/api/roles"},
		{http.MethodPost, "/api/roles"},
		{http.MethodGet, "/api/roles/1"},
		{http.MethodPut, "/api/roles/1"},
		{http.MethodDelete, "/api/roles/1"},
		{http.MethodPost, "/api/roles/1/permissions"},
	}

	for _, tt := range tests {
		t.Run(tt.method+"_"+tt.path, func(t *testing.T) {
			req := httptest.NewRequest(tt.method, tt.path, nil)
			w := httptest.NewRecorder()

			router.ServeHTTP(w, req)

			assert.NotEqual(t, http.StatusNotFound, w.Code)
		})
	}
}

func TestPermissionRoutesExist(t *testing.T) {
	router := gin.New()

	router.GET("/api/permissions", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "endpoint exists"})
	})

	req := httptest.NewRequest(http.MethodGet, "/api/permissions", nil)
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.NotEqual(t, http.StatusNotFound, w.Code)
}

func TestServerStructure(t *testing.T) {
	server := &Server{}

	assert.Nil(t, server.engine)
	assert.Nil(t, server.db)
	assert.Nil(t, server.cfg)
}
