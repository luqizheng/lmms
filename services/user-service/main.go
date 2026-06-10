package main

import (
	"os"
	"os/signal"
	"syscall"

	"superpowers/user-service/config"
	_ "superpowers/user-service/docs"
	"superpowers/user-service/models"
	"superpowers/user-service/store"

	"github.com/gin-gonic/gin"
	"go.uber.org/zap"
)

// @title User Service API
// @version 1.0
// @description 用户服务 API 文档
// @host localhost:8081
// @BasePath /
func main() {
	logger, err := zap.NewProduction()
	if err != nil {
		panic("failed to initialize logger")
	}
	defer logger.Sync()

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

	if err := db.AutoMigrate(
		&models.User{},
		&models.UserProfile{},
		&models.Role{},
		&models.Permission{},
		&models.UserRole{},
		&models.RolePermission{},
	); err != nil {
		sugar.Fatalf("Failed to migrate database: %v", err)
	}
	sugar.Info("Database connected and migrated successfully")

	server := &Server{
		cfg:    cfg,
		db:     db,
		engine: gin.Default(),
	}

	server.setupRoutes()

	consulRegistrar := NewConsulRegistrar(&cfg.Consul)
	sugar.Infof("Consul config: addr=%s service_name=%s service_id_file=%s service_id=%s",
		cfg.Consul.Addr, cfg.Consul.ServiceName, cfg.Consul.ServiceIDFile, consulRegistrar.ServiceID())

	quit := make(chan os.Signal, 1)
	signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		addr := cfg.Server.GetAddr()
		sugar.Infof("user-service starting on %s", addr)
		if err := server.engine.Run(addr); err != nil {
			sugar.Fatalf("Failed to start server: %v", err)
		}
	}()

	if err := consulRegistrar.Register(cfg.Server.Port); err != nil {
		sugar.Warnf("Warning: failed to register with consul: %v", err)
	} else {
		sugar.Info("user-service registered with consul")
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
	engine *gin.Engine
}
