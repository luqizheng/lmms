package handlers

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"superpowers/auth-service/config"

	"github.com/gin-gonic/gin"
	"github.com/redis/go-redis/v9"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"golang.org/x/crypto/bcrypt"
)

func init() {
	gin.SetMode(gin.TestMode)
}

func getRedisClient() *redis.Client {
	return redis.NewClient(&redis.Options{
		Addr: "127.0.0.1:6379",
		DB:   1,
	})
}

func skipIfRedisUnavailable(t *testing.T) {
	rdb := getRedisClient()
	defer rdb.Close()

	ctx := context.Background()
	if err := rdb.Ping(ctx).Err(); err != nil {
		t.Skip("Redis not available, skipping test")
	}
}

func setupTestRouter() *gin.Engine {
	router := gin.New()
	return router
}

func TestRegisterValidation(t *testing.T) {
	tests := []struct {
		name       string
		body       map[string]interface{}
		wantStatus int
	}{
		{
			name:       "missing username",
			body:       map[string]interface{}{"password": "password123"},
			wantStatus: http.StatusBadRequest,
		},
		{
			name:       "username too short",
			body:       map[string]interface{}{"username": "ab", "password": "password123"},
			wantStatus: http.StatusBadRequest,
		},
		{
			name:       "password too short",
			body:       map[string]interface{}{"username": "testuser", "password": "12345"},
			wantStatus: http.StatusBadRequest,
		},
		{
			name:       "missing password",
			body:       map[string]interface{}{"username": "testuser"},
			wantStatus: http.StatusBadRequest,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			router := setupTestRouter()
			body, _ := json.Marshal(tt.body)
			req := httptest.NewRequest(http.MethodPost, "/register", bytes.NewReader(body))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			router.POST("/register", Register(nil))
			router.ServeHTTP(w, req)

			assert.Equal(t, tt.wantStatus, w.Code)
		})
	}
}

func TestLoginValidation(t *testing.T) {
	tests := []struct {
		name       string
		body       map[string]interface{}
		wantStatus int
	}{
		{
			name:       "missing username",
			body:       map[string]interface{}{"password": "password123"},
			wantStatus: http.StatusBadRequest,
		},
		{
			name:       "missing password",
			body:       map[string]interface{}{"username": "testuser"},
			wantStatus: http.StatusBadRequest,
		},
		{
			name:       "empty body",
			body:       map[string]interface{}{},
			wantStatus: http.StatusBadRequest,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			router := setupTestRouter()
			body, _ := json.Marshal(tt.body)
			req := httptest.NewRequest(http.MethodPost, "/login", bytes.NewReader(body))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			router.POST("/login", Login(nil, nil, nil))
			router.ServeHTTP(w, req)

			assert.Equal(t, tt.wantStatus, w.Code)
		})
	}
}

func TestLogoutSuccess(t *testing.T) {
	router := setupTestRouter()

	req := httptest.NewRequest(http.MethodPost, "/logout", nil)
	w := httptest.NewRecorder()

	router.POST("/logout", Logout(nil, nil))
	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusOK, w.Code)

	var resp map[string]string
	err := json.Unmarshal(w.Body.Bytes(), &resp)
	require.NoError(t, err)
	assert.Equal(t, "logout successful", resp["message"])
}

func TestBcryptPasswordHashing(t *testing.T) {
	password := "testPassword123"

	hash, err := bcrypt.GenerateFromPassword([]byte(password), bcrypt.DefaultCost)
	require.NoError(t, err)
	assert.NotEmpty(t, hash)

	err = bcrypt.CompareHashAndPassword(hash, []byte(password))
	assert.NoError(t, err)

	err = bcrypt.CompareHashAndPassword(hash, []byte("wrongpassword"))
	assert.Error(t, err)
}

func TestBcryptDifferentHashes(t *testing.T) {
	password := "samePassword"

	hash1, err := bcrypt.GenerateFromPassword([]byte(password), bcrypt.DefaultCost)
	require.NoError(t, err)

	hash2, err := bcrypt.GenerateFromPassword([]byte(password), bcrypt.DefaultCost)
	require.NoError(t, err)

	assert.NotEqual(t, string(hash1), string(hash2))

	err = bcrypt.CompareHashAndPassword(hash1, []byte(password))
	assert.NoError(t, err)
}

func TestRedisCodeStorage(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping Redis integration test in short mode")
	}

	skipIfRedisUnavailable(t)
	rdb := getRedisClient()
	ctx := context.Background()

	code := "test-auth-code-123"
	userID := uint(1)

	key := "auth:code:" + code
	err := rdb.Set(ctx, key, userID, 5*time.Minute).Err()
	require.NoError(t, err)

	stored, err := rdb.Get(ctx, key).Uint64()
	require.NoError(t, err)
	assert.Equal(t, uint64(userID), stored)

	ttl, err := rdb.TTL(ctx, key).Result()
	require.NoError(t, err)
	assert.True(t, ttl > 0 && ttl <= 5*time.Minute)

	rdb.Del(ctx, key)
}

func TestLoginLockedReturns423(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping Redis integration test in short mode")
	}
	skipIfRedisUnavailable(t)
	rdb := getRedisClient()
	defer rdb.Close()
	ctx := context.Background()

	user := fmt.Sprintf("locked-test-%d", time.Now().UnixNano())
	require.NoError(t, rdb.Set(ctx, lockKey(user), "1", time.Minute).Err())
	defer rdb.Del(ctx, lockKey(user))

	cfg := &config.Config{}
	cfg.LoginSecurity.ApplyDefaults()
	router := setupTestRouter()
	router.POST("/login", Login(nil, rdb, cfg))

	body, _ := json.Marshal(map[string]string{
		"username": user,
		"password": "anything",
	})
	req := httptest.NewRequest(http.MethodPost, "/login", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	assert.Equal(t, http.StatusLocked, w.Code)
	var resp map[string]interface{}
	require.NoError(t, json.Unmarshal(w.Body.Bytes(), &resp))
	assert.Equal(t, "account locked", resp["error"])
	assert.NotZero(t, resp["retry_after"])
}

func TestRegisterRequestBinding(t *testing.T) {
	tests := []struct {
		name       string
		body       string
		wantStatus int
	}{
		{
			name:       "invalid json",
			body:       `{invalid json}`,
			wantStatus: http.StatusBadRequest,
		},
		{
			name:       "missing password",
			body:       `{"username":"testuser"}`,
			wantStatus: http.StatusBadRequest,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			router := setupTestRouter()
			req := httptest.NewRequest(http.MethodPost, "/register", bytes.NewReader([]byte(tt.body)))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			router.POST("/register", Register(nil))
			router.ServeHTTP(w, req)

			assert.Equal(t, tt.wantStatus, w.Code)
		})
	}
}

func TestClearFailureRemovesKeys(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping Redis integration test in short mode")
	}
	skipIfRedisUnavailable(t)
	rdb := getRedisClient()
	defer rdb.Close()
	ctx := context.Background()

	user := fmt.Sprintf("clear-test-%d", time.Now().UnixNano())
	require.NoError(t, rdb.Set(ctx, failKey(user), 5, time.Minute).Err())
	require.NoError(t, rdb.Set(ctx, lockKey(user), "1", time.Minute).Err())

	clearFailure(ctx, rdb, user)

	failExists, _ := rdb.Exists(ctx, failKey(user)).Result()
	lockExists, _ := rdb.Exists(ctx, lockKey(user)).Result()
	assert.Equal(t, int64(0), failExists)
	assert.Equal(t, int64(0), lockExists)
}

func TestLoginEndToEndLockoutFlow(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping Redis integration test in short mode")
	}
	skipIfRedisUnavailable(t)
	rdb := getRedisClient()
	defer rdb.Close()
	ctx := context.Background()

	user := fmt.Sprintf("e2e-%d", time.Now().UnixNano())
	defer rdb.Del(ctx, failKey(user), lockKey(user))

	cfg := &config.Config{}
	cfg.LoginSecurity.MaxFailures = 3
	cfg.LoginSecurity.LockoutDuration = time.Minute
	cfg.LoginSecurity.CounterWindow = 2 * time.Minute

	router := setupTestRouter()
	router.POST("/login", Login(nil, rdb, cfg))

	body, _ := json.Marshal(map[string]string{
		"username": user,
		"password": "wrong",
	})

	for i := 1; i <= 3; i++ {
		req := httptest.NewRequest(http.MethodPost, "/login", bytes.NewReader(body))
		req.Header.Set("Content-Type", "application/json")
		w := httptest.NewRecorder()
		router.ServeHTTP(w, req)
		assert.Equal(t, http.StatusUnauthorized, w.Code, "attempt %d should be 401", i)
	}

	req := httptest.NewRequest(http.MethodPost, "/login", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)
	assert.Equal(t, http.StatusLocked, w.Code, "4th attempt should be 423")
}

func TestIncrFailureSetsLockAtThreshold(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping Redis integration test in short mode")
	}
	skipIfRedisUnavailable(t)
	rdb := getRedisClient()
	defer rdb.Close()
	ctx := context.Background()

	user := fmt.Sprintf("fail-test-%d", time.Now().UnixNano())
	defer rdb.Del(ctx, failKey(user), lockKey(user))

	n, locked, err := incrFailure(ctx, rdb, user, 3, time.Minute)
	require.NoError(t, err)
	assert.Equal(t, int64(1), n)
	assert.False(t, locked)

	n, locked, err = incrFailure(ctx, rdb, user, 3, time.Minute)
	require.NoError(t, err)
	assert.Equal(t, int64(2), n)
	assert.False(t, locked)

	n, locked, err = incrFailure(ctx, rdb, user, 3, time.Minute)
	require.NoError(t, err)
	assert.Equal(t, int64(3), n)
	assert.True(t, locked)
}

func TestLoginRequestBinding(t *testing.T) {
	tests := []struct {
		name       string
		body       string
		wantStatus int
	}{
		{
			name:       "missing password field",
			body:       `{"username":"testuser"}`,
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
			router := setupTestRouter()
			req := httptest.NewRequest(http.MethodPost, "/login", bytes.NewReader([]byte(tt.body)))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			router.POST("/login", Login(nil, nil, nil))
			router.ServeHTTP(w, req)

			assert.Equal(t, tt.wantStatus, w.Code)
		})
	}
}

func TestLoginCodeGeneration(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping Redis integration test in short mode")
	}

	skipIfRedisUnavailable(t)
	rdb := getRedisClient()
	ctx := context.Background()
	defer rdb.Close()

	code := "test-code-456"
	key := "auth:code:" + code

	userID := uint(99)
	err := rdb.Set(ctx, key, userID, 5*time.Minute).Err()
	require.NoError(t, err)

	exists, err := rdb.Exists(ctx, key).Result()
	require.NoError(t, err)
	assert.Equal(t, int64(1), exists)

	val, err := rdb.Get(ctx, key).Uint64()
	require.NoError(t, err)
	assert.Equal(t, uint64(userID), val)

	rdb.Del(ctx, key)
}
