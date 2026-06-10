package store

import (
	"context"
	"database/sql"
	"fmt"
	"strings"

	"superpowers/user-service/config"

	_ "modernc.org/sqlite"
	"gorm.io/driver/mysql"
	"gorm.io/driver/postgres"
	"gorm.io/driver/sqlserver"
	"gorm.io/gorm"
	"gorm.io/gorm/clause"
	"gorm.io/gorm/logger"
	"gorm.io/gorm/schema"
)

type DB struct {
	*gorm.DB
}

type sqliteDialector struct {
	conn *sql.DB
}

func (d sqliteDialector) Name() string {
	return "sqlite"
}

func (d sqliteDialector) DB() (*sql.DB, error) {
	return d.conn, nil
}

func (d sqliteDialector) BindVarTo(writer clause.Writer, stmt *gorm.Statement, v interface{}) {
	writer.WriteByte('?')
}

func (d sqliteDialector) QuoteTo(writer clause.Writer, str string) {
	writer.WriteByte('"')
	writer.WriteString(str)
	writer.WriteByte('"')
}

func (d sqliteDialector) Explain(sql string, vars ...interface{}) string {
	return sql
}

func (d sqliteDialector) DataTypeOf(field *schema.Field) string {
	return ""
}

func (d sqliteDialector) SavePoint(tx *gorm.DB, name string) error {
	return nil
}

func (d sqliteDialector) RollbackTo(tx *gorm.DB, name string) error {
	return nil
}

func (d sqliteDialector) DefaultValueOf(field *schema.Field) clause.Expression {
	return nil
}

func (d sqliteDialector) Initialize(db *gorm.DB) error {
	return nil
}

func (d sqliteDialector) Migrator(db *gorm.DB) gorm.Migrator {
	return db.Session(&gorm.Session{}).Migrator()
}

func NewDB(cfg *config.DatabaseConfig) (*DB, error) {
	dsn := cfg.GetDSN()

	var dialector gorm.Dialector
	switch strings.ToLower(cfg.Type) {
	case "mysql":
		dialector = mysql.Open(dsn)
	case "mssql", "sqlserver":
		dialector = sqlserver.Open(dsn)
	case "postgres", "postgresql":
		dialector = postgres.Open(dsn)
	case "sqlite":
		sqlDB, err := sql.Open("sqlite", dsn)
		if err != nil {
			return nil, fmt.Errorf("failed to open sqlite database: %w", err)
		}
		dialector = sqliteDialector{conn: sqlDB}
	case "dm":
		return nil, fmt.Errorf("dm driver not installed. Please install DM driver manually: go get github.com/daMengOpen/dmdb")
	default:
		dialector = mysql.Open(dsn)
	}

	db, err := gorm.Open(dialector, &gorm.Config{
		Logger: logger.Default.LogMode(logger.Info),
	})
	if err != nil {
		return nil, fmt.Errorf("failed to connect to database: %w", err)
	}

	sqlDB, err := db.DB()
	if err != nil {
		return nil, fmt.Errorf("failed to get underlying sql.DB: %w", err)
	}

	sqlDB.SetMaxIdleConns(10)
	sqlDB.SetMaxOpenConns(100)

	return &DB{DB: db}, nil
}

func (db *DB) Close() error {
	sqlDB, err := db.DB.DB()
	if err != nil {
		return err
	}
	return sqlDB.Close()
}

func (db *DB) Ping(ctx context.Context) error {
	sqlDB, err := db.DB.DB()
	if err != nil {
		return err
	}
	return sqlDB.PingContext(ctx)
}
