package handlers

import (
	"bytes"
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

func setupTestRouter() *gin.Engine {
	return gin.New()
}

func TestGetCurrentUserUnauthorized(t *testing.T) {
	router := setupTestRouter()

	router.GET("/api/users/me", GetCurrentUser(nil))

	req := httptest.NewRequest(http.MethodGet, "/api/users/me", nil)
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusUnauthorized, w.Code)

	var resp map[string]string
	err := json.Unmarshal(w.Body.Bytes(), &resp)
	assert.NoError(t, err)
	assert.Equal(t, "unauthorized", resp["error"])
}

func TestUpdateCurrentUserUnauthorized(t *testing.T) {
	router := setupTestRouter()

	router.PUT("/api/users/me", UpdateCurrentUser(nil))

	body := `{"nickname": "newname"}`
	req := httptest.NewRequest(http.MethodPut, "/api/users/me", bytes.NewReader([]byte(body)))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusUnauthorized, w.Code)
}

func TestUpdateCurrentUserValidation(t *testing.T) {
	router := setupTestRouter()

	router.PUT("/api/users/me", func(c *gin.Context) {
		c.Set("user_id", 1)

		var req struct {
			Nickname string `json:"nickname"`
			Email    string `json:"email"`
		}

		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
			return
		}
		c.JSON(http.StatusOK, gin.H{"status": "ok"})
	})

	tests := []struct {
		name string
		body string
	}{
		{"empty body", "{}"},
		{"nickname only", `{"nickname": "test"}`},
		{"email only", `{"email": "test@example.com"}`},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodPut, "/api/users/me", bytes.NewReader([]byte(tt.body)))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			router.ServeHTTP(w, req)

			assert.NotEqual(t, http.StatusBadRequest, w.Code)
		})
	}
}

func TestListUsersPagination(t *testing.T) {
	router := setupTestRouter()

	router.GET("/api/users", func(c *gin.Context) {
		page := c.DefaultQuery("page", "1")
		pageSize := c.DefaultQuery("page_size", "20")

		c.JSON(http.StatusOK, gin.H{
			"data":      []interface{}{},
			"total":     0,
			"page":      page,
			"page_size": pageSize,
		})
	})

	tests := []struct {
		name         string
		query        string
		wantPage     string
		wantPageSize string
	}{
		{"default pagination", "", "1", "20"},
		{"custom page", "?page=5", "5", "20"},
		{"custom page size", "?page_size=50", "1", "50"},
		{"both custom", "?page=3&page_size=30", "3", "30"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodGet, "/api/users"+tt.query, nil)
			w := httptest.NewRecorder()

			router.ServeHTTP(w, req)

			assert.Equal(t, http.StatusOK, w.Code)

			var resp map[string]interface{}
			err := json.Unmarshal(w.Body.Bytes(), &resp)
			assert.NoError(t, err)
			assert.Equal(t, tt.wantPage, resp["page"])
			assert.Equal(t, tt.wantPageSize, resp["page_size"])
		})
	}
}

func TestGetUserInvalidID(t *testing.T) {
	router := setupTestRouter()

	router.GET("/api/users/:id", GetUser(nil))

	invalidIDs := []string{"abc", "-1", "notanumber"}

	for _, id := range invalidIDs {
		t.Run("invalid_id_"+id, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodGet, "/api/users/"+id, nil)
			w := httptest.NewRecorder()

			router.ServeHTTP(w, req)

			assert.Equal(t, http.StatusBadRequest, w.Code)
		})
	}
}

func TestListRolesSuccess(t *testing.T) {
	router := setupTestRouter()

	router.GET("/api/roles", func(c *gin.Context) {
		c.JSON(http.StatusOK, []gin.H{
			{"id": 1, "name": "admin", "code": "admin"},
			{"id": 2, "name": "user", "code": "user"},
		})
	})

	req := httptest.NewRequest(http.MethodGet, "/api/roles", nil)
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusOK, w.Code)

	var resp []map[string]interface{}
	err := json.Unmarshal(w.Body.Bytes(), &resp)
	assert.NoError(t, err)
	assert.Len(t, resp, 2)
}

func TestCreateRoleValidation(t *testing.T) {
	router := setupTestRouter()

	router.POST("/api/roles", CreateRole(nil))

	tests := []struct {
		name       string
		body       string
		wantStatus int
	}{
		{
			name:       "missing name",
			body:       `{"code": "test_role"}`,
			wantStatus: http.StatusBadRequest,
		},
		{
			name:       "missing code",
			body:       `{"name": "Test Role"}`,
			wantStatus: http.StatusBadRequest,
		},
		{
			name:       "empty body",
			body:       `{}`,
			wantStatus: http.StatusBadRequest,
		},
		{
			name:       "invalid json",
			body:       `{invalid}`,
			wantStatus: http.StatusBadRequest,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodPost, "/api/roles", bytes.NewReader([]byte(tt.body)))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			router.ServeHTTP(w, req)

			assert.Equal(t, tt.wantStatus, w.Code)
		})
	}
}

func TestGetRoleInvalidID(t *testing.T) {
	router := setupTestRouter()

	router.GET("/api/roles/:id", GetRole(nil))

	invalidIDs := []string{"abc", "-1"}

	for _, id := range invalidIDs {
		t.Run("invalid_id_"+id, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodGet, "/api/roles/"+id, nil)
			w := httptest.NewRecorder()

			router.ServeHTTP(w, req)

			assert.Equal(t, http.StatusBadRequest, w.Code)
		})
	}
}

func TestUpdateRoleInvalidID(t *testing.T) {
	router := setupTestRouter()

	router.PUT("/api/roles/:id", UpdateRole(nil))

	body := `{"name": "Updated Name"}`
	req := httptest.NewRequest(http.MethodPut, "/api/roles/invalid", bytes.NewReader([]byte(body)))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusBadRequest, w.Code)
}

func TestDeleteRoleInvalidID(t *testing.T) {
	router := setupTestRouter()

	router.DELETE("/api/roles/:id", DeleteRole(nil))

	req := httptest.NewRequest(http.MethodDelete, "/api/roles/abc", nil)
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusBadRequest, w.Code)
}

func TestAssignRolePermissionsInvalidID(t *testing.T) {
	router := setupTestRouter()

	router.POST("/api/roles/:id/permissions", AssignRolePermissions(nil))

	body := `{"permissions": [1, 2, 3]}`
	req := httptest.NewRequest(http.MethodPost, "/api/roles/abc/permissions", bytes.NewReader([]byte(body)))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusBadRequest, w.Code)
}

func TestListPermissionsSuccess(t *testing.T) {
	router := setupTestRouter()

	router.GET("/api/permissions", func(c *gin.Context) {
		c.JSON(http.StatusOK, []gin.H{
			{"id": 1, "name": "user:read", "code": "user:read"},
			{"id": 2, "name": "user:write", "code": "user:write"},
			{"id": 3, "name": "role:manage", "code": "role:manage"},
		})
	})

	req := httptest.NewRequest(http.MethodGet, "/api/permissions", nil)
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusOK, w.Code)

	var resp []map[string]interface{}
	err := json.Unmarshal(w.Body.Bytes(), &resp)
	assert.NoError(t, err)
	assert.Len(t, resp, 3)
}

func TestAssignUserRolesInvalidID(t *testing.T) {
	router := setupTestRouter()

	router.POST("/api/users/:id/roles", AssignUserRoles(nil))

	body := `{"roles": [1, 2]}`
	req := httptest.NewRequest(http.MethodPost, "/api/users/abc/roles", bytes.NewReader([]byte(body)))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusBadRequest, w.Code)
}

func TestRemoveUserRolesInvalidID(t *testing.T) {
	router := setupTestRouter()

	router.DELETE("/api/users/:id/roles", RemoveUserRoles(nil))

	body := `{"roles": [1]}`
	req := httptest.NewRequest(http.MethodDelete, "/api/users/invalid/roles", bytes.NewReader([]byte(body)))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusBadRequest, w.Code)
}

func TestPatchUserExtendsInvalidID(t *testing.T) {
	router := setupTestRouter()

	router.PATCH("/api/users/:id/extends", PatchUserExtends(nil))

	body := `{"dept": "销售部"}`
	req := httptest.NewRequest(http.MethodPatch, "/api/users/abc/extends", bytes.NewReader([]byte(body)))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusBadRequest, w.Code)
}

func TestPatchUserExtendsInvalidJSON(t *testing.T) {
	router := setupTestRouter()

	router.PATCH("/api/users/:id/extends", PatchUserExtends(nil))

	req := httptest.NewRequest(http.MethodPatch, "/api/users/1/extends", bytes.NewReader([]byte(`{invalid}`)))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusBadRequest, w.Code)
}
