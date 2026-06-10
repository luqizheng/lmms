package handlers

import (
	"context"
	"fmt"
	"net/http"
	"time"

	"superpowers/auth-service/config"
	"superpowers/auth-service/models"
	"superpowers/auth-service/store"

	"github.com/gin-gonic/gin"
	"github.com/google/uuid"
	"github.com/redis/go-redis/v9"
	"golang.org/x/crypto/bcrypt"
	"gorm.io/gorm"
)

const dummyBcryptHash = "$2a$10$N9qo8uLOickgx2ZMRZoMyeIjZAgcfl7p92ldGxad68LJZdL17lhWy" // hash of "password"

var (
	failKeyPrefix = "auth:login:fail:"
	lockKeyPrefix = "auth:login:lock:"
)

func failKey(username string) string { return failKeyPrefix + username }
func lockKey(username string) string { return lockKeyPrefix + username }

func isLocked(ctx context.Context, rdb *redis.Client, username string) (bool, time.Duration, error) {
	ttl, err := rdb.TTL(ctx, lockKey(username)).Result()
	if err != nil {
		return false, 0, err
	}
	if ttl > 0 {
		return true, ttl, nil
	}
	return false, 0, nil
}

func incrFailure(ctx context.Context, rdb *redis.Client, username string, max int, window time.Duration) (int64, bool, error) {
	n, err := rdb.Incr(ctx, failKey(username)).Result()
	if err != nil {
		return 0, false, err
	}
	if n == 1 {
		if err := rdb.Expire(ctx, failKey(username), window).Err(); err != nil {
			return n, false, err
		}
	}
	locked := n >= int64(max)
	return n, locked, nil
}

func setLock(ctx context.Context, rdb *redis.Client, username string, d time.Duration) error {
	return rdb.Set(ctx, lockKey(username), "1", d).Err()
}

func clearFailure(ctx context.Context, rdb *redis.Client, username string) {
	rdb.Del(ctx, failKey(username), lockKey(username))
}

func Register(db *store.DB) gin.HandlerFunc {
	return func(c *gin.Context) {
		var req struct {
			Username string `json:"username" binding:"required,min=3,max=64"`
			Password string `json:"password" binding:"required,min=6"`
			Email    string `json:"email"`
			Phone    string `json:"phone"`
			Nickname string `json:"nickname"`
		}

		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
			return
		}

		hash, err := bcrypt.GenerateFromPassword([]byte(req.Password), bcrypt.DefaultCost)
		if err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to hash password"})
			return
		}

		user := models.User{
			Username:     req.Username,
			PasswordHash: string(hash),
			Email:        req.Email,
			Phone:        req.Phone,
			Nickname:     req.Nickname,
			Status:       1,
		}

		if err := db.DB.Create(&user).Error; err != nil {
			c.JSON(http.StatusConflict, gin.H{"error": "username already exists"})
			return
		}

		c.JSON(http.StatusCreated, gin.H{
			"id":       user.ID,
			"username": user.Username,
		})
	}
}

func Login(db *store.DB, rdb *redis.Client, cfg *config.Config) gin.HandlerFunc {
	return func(c *gin.Context) {
		var req struct {
			Username string `json:"username" binding:"required"`
			Password string `json:"password" binding:"required"`
		}

		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
			return
		}

		ctx := context.Background()
		if locked, retryAfter, lerr := isLocked(ctx, rdb, req.Username); lerr == nil && locked {
			if db != nil {
				db.DB.Create(&models.LoginLog{
					UserID:     nil,
					Username:   req.Username,
					IP:         c.ClientIP(),
					UserAgent:  c.GetHeader("User-Agent"),
					Success:    false,
					FailReason: "locked",
					LoginAt:    time.Now(),
				})
			}
			c.JSON(http.StatusLocked, gin.H{
				"error":       "account locked",
				"retry_after": int(retryAfter.Seconds()),
			})
			return
		}

		var user models.User
		var userErr error
		if db != nil {
			userErr = db.DB.Where("username = ?", req.Username).First(&user).Error
		} else {
			userErr = gorm.ErrRecordNotFound
		}

		if userErr != nil {
			_ = bcrypt.CompareHashAndPassword([]byte(dummyBcryptHash), []byte(req.Password))
			n, _, ierr := incrFailure(ctx, rdb, req.Username,
				cfg.LoginSecurity.MaxFailures, cfg.LoginSecurity.CounterWindow)
			if ierr == nil && n >= int64(cfg.LoginSecurity.MaxFailures) {
				_ = setLock(ctx, rdb, req.Username, cfg.LoginSecurity.LockoutDuration)
			}
			if db != nil {
				_ = db.DB.Create(&models.LoginLog{
					UserID:     nil,
					Username:   req.Username,
					IP:         c.ClientIP(),
					UserAgent:  c.GetHeader("User-Agent"),
					Success:    false,
					FailReason: "user_not_found",
					LoginAt:    time.Now(),
				}).Error
			}
			c.JSON(http.StatusUnauthorized, gin.H{"error": "invalid credentials"})
			return
		}

		if err := bcrypt.CompareHashAndPassword([]byte(user.PasswordHash), []byte(req.Password)); err != nil {
			n, _, ierr := incrFailure(ctx, rdb, req.Username,
				cfg.LoginSecurity.MaxFailures, cfg.LoginSecurity.CounterWindow)
			if ierr == nil && n >= int64(cfg.LoginSecurity.MaxFailures) {
				_ = setLock(ctx, rdb, req.Username, cfg.LoginSecurity.LockoutDuration)
			}
			if db != nil {
				_ = db.DB.Create(&models.LoginLog{
					UserID:     nil,
					Username:   req.Username,
					IP:         c.ClientIP(),
					UserAgent:  c.GetHeader("User-Agent"),
					Success:    false,
					FailReason: "bad_password",
					LoginAt:    time.Now(),
				}).Error
			}
			c.JSON(http.StatusUnauthorized, gin.H{"error": "invalid credentials"})
			return
		}

		code := uuid.New().String()
		key := fmt.Sprintf("auth:code:%s", code)
		if err := rdb.Set(ctx, key, user.ID, 5*time.Minute).Err(); err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to generate code"})
			return
		}

		clearFailure(ctx, rdb, req.Username)

		now := time.Now()
		ip := c.ClientIP()
		ua := c.GetHeader("User-Agent")
		uid := user.ID

		if db != nil {
			if err := db.DB.Create(&models.LoginLog{
				UserID:    &uid,
				Username:  req.Username,
				IP:        ip,
				UserAgent: ua,
				Success:   true,
				LoginAt:   now,
			}).Error; err != nil {
				_ = err
			}
			db.DB.Model(&user).Updates(map[string]interface{}{
				"last_login_at": now,
				"last_login_ip": ip,
			})
		}

		c.JSON(http.StatusOK, gin.H{
			"code":       code,
			"expires_in": 300,
		})
	}
}

func Logout(rdb *redis.Client, cfg *config.Config) gin.HandlerFunc {
	return func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "logout successful"})
	}
}
