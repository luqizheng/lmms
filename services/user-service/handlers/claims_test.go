package handlers

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/stretchr/testify/assert"
	"superpowers/user-service/models"
)

// TestBuildUserClaimsRoles 纯函数：roles + permissions 聚合去重。
func TestBuildUserClaimsRoles(t *testing.T) {
	roles := []models.Role{
		{Code: "sales", Permissions: []models.Permission{
			{Code: "contract.read"},
			{Code: "contract.write"},
		}},
		{Code: "manager", Permissions: []models.Permission{
			{Code: "contract.read"}, // 重复，期望去重
			{Code: "user.read"},
		}},
	}
	got := buildUserClaimsFromRoles(roles)
	assert.ElementsMatch(t, []string{"sales", "manager"}, got["roles"])
	assert.ElementsMatch(t, []string{"contract.read", "contract.write", "user.read"}, got["permissions"])
}

// TestBuildUserClaimsEmpty roles 为空时返回空切片。
func TestBuildUserClaimsEmpty(t *testing.T) {
	got := buildUserClaimsFromRoles(nil)
	assert.Equal(t, []string{}, got["roles"])
	assert.Equal(t, []string{}, got["permissions"])
}

// TestGetUserClaimsInvalidID 路径：非数字 ID 返回 400。
func TestGetUserClaimsInvalidID(t *testing.T) {
	gin.SetMode(gin.TestMode)
	r := gin.New()
	r.GET("/internal/users/:id/claims", GetUserClaims(nil))

	req := httptest.NewRequest(http.MethodGet, "/internal/users/abc/claims", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	assert.Equal(t, http.StatusBadRequest, w.Code)
	var body map[string]string
	assert.NoError(t, json.Unmarshal(w.Body.Bytes(), &body))
	assert.Equal(t, "invalid_id", body["error"])
}
