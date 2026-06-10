-- 2026-06-10 user-service 密码登录：加 password_hash / last_login_*
-- 新增 user_login_log 表记录成功 / 失败登录

USE oa3;

ALTER TABLE users
  ADD COLUMN password_hash VARCHAR(255) NULL AFTER user_code,
  ADD COLUMN last_login_at DATETIME NULL,
  ADD COLUMN last_login_ip VARCHAR(45) NULL;

CREATE TABLE IF NOT EXISTS user_login_log (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  user_id BIGINT UNSIGNED NULL,
  username VARCHAR(64) NOT NULL,
  ip VARCHAR(45) NOT NULL,
  user_agent VARCHAR(255) NOT NULL,
  success TINYINT(1) NOT NULL,
  fail_reason VARCHAR(32) NULL,
  login_at DATETIME(3) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  INDEX idx_login_log_username (username),
  INDEX idx_login_log_user_id (user_id),
  INDEX idx_login_log_success (success),
  INDEX idx_login_log_login_at (login_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
