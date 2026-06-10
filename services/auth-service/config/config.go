package config

import (
	"fmt"
	"strings"
	"time"

	"github.com/spf13/viper"
)

type Config struct {
	Server        ServerConfig        `mapstructure:"server"`
	Database      DatabaseConfig      `mapstructure:"database"`
	Redis         RedisConfig         `mapstructure:"redis"`
	TokenServer   TokenServerConfig   `mapstructure:"token_server"`
	Consul        ConsulConfig        `mapstructure:"consul"`
	LoginSecurity LoginSecurityConfig `mapstructure:"login_security"`
}

type ConsulConfig struct {
	Addr          string `mapstructure:"addr"`
	ServiceName   string `mapstructure:"service_name"`
	ServiceIDFile string `mapstructure:"service_id_file"`
}

type ServerConfig struct {
	Host string `mapstructure:"host"`
	Port int    `mapstructure:"port"`
}

type DatabaseConfig struct {
	Type     string `mapstructure:"type"`
	Host     string `mapstructure:"host"`
	Port     int    `mapstructure:"port"`
	User     string `mapstructure:"user"`
	Password string `mapstructure:"password"`
	Name     string `mapstructure:"name"`
	DSN      string `mapstructure:"dsn"`
}

type RedisConfig struct {
	Addr     string `mapstructure:"addr"`
	Password string `mapstructure:"password"`
	DB       int    `mapstructure:"db"`
}

type TokenServerConfig struct {
	URL string `mapstructure:"url"`
}

type LoginSecurityConfig struct {
	MaxFailures     int           `mapstructure:"max_failures"`
	LockoutDuration time.Duration `mapstructure:"lockout_duration"`
	CounterWindow   time.Duration `mapstructure:"counter_window"`
}

func (c *LoginSecurityConfig) ApplyDefaults() {
	if c.MaxFailures <= 0 {
		c.MaxFailures = 6
	}
	if c.LockoutDuration <= 0 {
		c.LockoutDuration = 5 * time.Minute
	}
	if c.CounterWindow <= 0 {
		c.CounterWindow = 15 * time.Minute
	}
}

func (d *DatabaseConfig) GetDSN() string {
	if d.DSN != "" {
		return d.DSN
	}

	switch strings.ToLower(d.Type) {
	case "mysql":
		return fmt.Sprintf("%s:%s@tcp(%s:%d)/%s?charset=utf8mb4&parseTime=True&loc=Local",
			d.User, d.Password, d.Host, d.Port, d.Name)
	case "mssql":
		return fmt.Sprintf("sqlserver://%s:%s@%s:%d?database=%s",
			d.User, d.Password, d.Host, d.Port, d.Name)
	case "dm":
		return fmt.Sprintf("dm://%s:%s@%s:%d",
			d.User, d.Password, d.Host, d.Port)
	default:
		return fmt.Sprintf("%s:%s@tcp(%s:%d)/%s?charset=utf8mb4&parseTime=True&loc=Local",
			d.User, d.Password, d.Host, d.Port, d.Name)
	}
}

func (s *ServerConfig) GetAddr() string {
	return fmt.Sprintf("%s:%d", s.Host, s.Port)
}

func Load(configPath string) (*Config, error) {
	viper.SetConfigFile(configPath)
	viper.SetConfigType("yaml")

	viper.AutomaticEnv()
	viper.SetEnvKeyReplacer(strings.NewReplacer(".", "_"))

	if err := viper.ReadInConfig(); err != nil {
		if _, ok := err.(viper.ConfigFileNotFoundError); ok {
		} else {
			return nil, fmt.Errorf("failed to read config: %w", err)
		}
	}

	var cfg Config
	if err := viper.Unmarshal(&cfg); err != nil {
		return nil, fmt.Errorf("failed to unmarshal config: %w", err)
	}

	cfg.LoginSecurity.ApplyDefaults()
	if cfg.LoginSecurity.CounterWindow < cfg.LoginSecurity.LockoutDuration {
		return nil, fmt.Errorf("login_security.counter_window must be >= lockout_duration")
	}

	return &cfg, nil
}
