package store

import (
	"testing"

	"superpowers/user-service/config"

	"github.com/stretchr/testify/assert"
)

func TestNewDB_InvalidConfig(t *testing.T) {
	cfg := &config.DatabaseConfig{
		Type: "unknown",
	}

	db, err := NewDB(cfg)
	assert.Nil(t, db)
	assert.Error(t, err)
}

func TestNewDB_DMNotSupported(t *testing.T) {
	cfg := &config.DatabaseConfig{
		Type: "dm",
	}

	db, err := NewDB(cfg)
	assert.Nil(t, db)
	assert.Error(t, err)
	assert.Contains(t, err.Error(), "dm driver not installed")
}

func TestDatabaseConfig_GetDSN_MySQL(t *testing.T) {
	cfg := &config.DatabaseConfig{
		Type:     "mysql",
		Host:     "localhost",
		Port:     3306,
		User:     "root",
		Password: "password",
		Name:     "testdb",
	}

	dsn := cfg.GetDSN()
	assert.Contains(t, dsn, "root:password@tcp(localhost:3306)/testdb")
}

func TestDatabaseConfig_GetDSN_SQLite(t *testing.T) {
	cfg := &config.DatabaseConfig{
		Type: "sqlite",
		Name: "./test.db",
	}

	dsn := cfg.GetDSN()
	assert.Equal(t, "./test.db", dsn)
}

func TestDatabaseConfig_GetDSN_SQLiteDefault(t *testing.T) {
	cfg := &config.DatabaseConfig{
		Type: "sqlite",
	}

	dsn := cfg.GetDSN()
	assert.Equal(t, "./test.db", dsn)
}