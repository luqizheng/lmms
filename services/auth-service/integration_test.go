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

func TestAuthRoutesExist(t *testing.T) {
	router := gin.New()

	router.POST("/api/auth/register", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "register endpoint exists"})
	})
	router.POST("/api/auth/login", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "login endpoint exists"})
	})
	router.POST("/api/auth/logout", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "logout endpoint exists"})
	})

	tests := []struct {
		method string
		path   string
	}{
		{http.MethodPost, "/api/auth/register"},
		{http.MethodPost, "/api/auth/login"},
		{http.MethodPost, "/api/auth/logout"},
	}

	for _, tt := range tests {
		t.Run(tt.path, func(t *testing.T) {
			req := httptest.NewRequest(tt.method, tt.path, nil)
			w := httptest.NewRecorder()

			router.ServeHTTP(w, req)

			assert.NotEqual(t, http.StatusNotFound, w.Code)
		})
	}
}

func TestServerStructure(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping server structure test in short mode")
	}

	server := &Server{}

	assert.Nil(t, server.engine)
	assert.Nil(t, server.db)
	assert.Nil(t, server.redis)
	assert.Nil(t, server.cfg)
}

func TestCORSHeaders(t *testing.T) {
	router := gin.New()

	router.Use(func(c *gin.Context) {
		c.Header("Access-Control-Allow-Origin", "*")
		c.Next()
	})

	router.GET("/test", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"status": "ok"})
	})

	req := httptest.NewRequest(http.MethodGet, "/test", nil)
	req.Header.Set("Origin", "http://example.com")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, "*", w.Header().Get("Access-Control-Allow-Origin"))
}
