package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"syscall"
	"time"

	"superpowers/auth-service/config"
	"superpowers/auth-service/models"
	"superpowers/auth-service/store"

	"github.com/gin-gonic/gin"
	"github.com/redis/go-redis/v9"
	"go.uber.org/zap"
)

func main() {
	logger, err := zap.NewProduction()
	if err != nil {
		panic("failed to initialize logger")
	}
	defer logger.Sync()
	zap.ReplaceGlobals(logger)

	sugar := logger.Sugar()

	cfg, err := config.Load("config/config.yaml")
	if err != nil {
		sugar.Fatalf("Failed to load config: %v", err)
	}

	sugar.Infof("Database config: type=%s host=%s port=%d name=%s user=%s",
		cfg.Database.Type, cfg.Database.Host, cfg.Database.Port, cfg.Database.Name, cfg.Database.User)

	db, err := store.NewDB(&cfg.Database)
	if err != nil {
		sugar.Fatalf("Failed to connect to database: %v", err)
	}
	defer db.Close()

	if err := db.AutoMigrate(&models.User{}, &models.LoginLog{}); err != nil {
		sugar.Fatalf("Failed to migrate database: %v", err)
	}
	sugar.Info("Database connected and migrated successfully")

	rdb := redis.NewClient(&redis.Options{
		Addr:     cfg.Redis.Addr,
		Password: cfg.Redis.Password,
		DB:       cfg.Redis.DB,
	})

	sugar.Infof("Redis config: addr=%s db=%d", cfg.Redis.Addr, cfg.Redis.DB)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	if err := rdb.Ping(ctx).Err(); err != nil {
		sugar.Fatalf("Failed to connect to Redis: %v", err)
	}
	sugar.Info("Redis connected successfully")

	server := &Server{
		cfg:    cfg,
		db:     db,
		redis:  rdb,
		engine: gin.Default(),
	}

	server.engine.SetTrustedProxies([]string{"0.0.0.0/0", "::/0"})

	server.setupRoutes()

	consulRegistrar := NewConsulRegistrar(&cfg.Consul)
	sugar.Infof("Consul config: addr=%s service_name=%s service_id_file=%s service_id=%s",
		cfg.Consul.Addr, cfg.Consul.ServiceName, cfg.Consul.ServiceIDFile, consulRegistrar.ServiceID())

	quit := make(chan os.Signal, 1)
	signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		addr := cfg.Server.GetAddr()
		sugar.Infof("auth-service starting on %s", addr)
		if err := server.engine.Run(addr); err != nil {
			sugar.Fatalf("Failed to start server: %v", err)
		}
	}()

	if host, err := consulRegistrar.Register(cfg.Server.Port); err != nil {
		sugar.Warnf("Warning: failed to register with consul: %v", err)
	} else {
		sugar.Infof("auth-service registered with consul: name=%s id=%s address=%s:%d health=%s",
			cfg.Consul.ServiceName, consulRegistrar.ServiceID(), host, cfg.Server.Port,
			fmt.Sprintf("http://%s:%d/health", host, cfg.Server.Port))
	}

	<-quit
	sugar.Info("Shutting down server...")

	if err := consulRegistrar.Deregister(); err != nil {
		sugar.Warnf("Warning: failed to deregister from consul: %v", err)
	}
}

type Server struct {
	cfg    *config.Config
	db     *store.DB
	redis  *redis.Client
	engine *gin.Engine
}
