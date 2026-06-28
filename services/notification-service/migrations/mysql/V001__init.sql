-- notification-service 表结构（共享 `superpowers` 数据库；不含 CREATE DATABASE / USE）。
-- 表前缀 `ntf_`。

CREATE TABLE IF NOT EXISTS ntf_announcements (
  id              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  title           VARCHAR(200)    NOT NULL,
  body            MEDIUMTEXT      NOT NULL,
  publish_at      DATETIME        NOT NULL,
  expire_at       DATETIME        NULL,
  pinned          TINYINT(1)      NOT NULL DEFAULT 0,
  popover         TINYINT(1)      NOT NULL DEFAULT 0,
  status          ENUM('draft','scheduled','published','revoked') NOT NULL,
  created_by      VARCHAR(32)     NOT NULL,
  created_at      DATETIME        NOT NULL,
  PRIMARY KEY (id),
  KEY idx_ntf_announcements_status_publish (status, publish_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS ntf_announcement_recipients (
  announcement_id BIGINT      NOT NULL,
  scope_type      ENUM('user','role','dept') NOT NULL,
  scope_id        VARCHAR(32) NOT NULL,
  PRIMARY KEY (announcement_id, scope_type, scope_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS ntf_announcement_deliveries (
  announcement_id BIGINT      NOT NULL,
  user_id         VARCHAR(32) NOT NULL,
  delivered_at    DATETIME    NOT NULL,
  PRIMARY KEY (announcement_id, user_id),
  KEY idx_ntf_announcement_deliveries_user (user_id, announcement_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS ntf_announcement_reads (
  announcement_id BIGINT      NOT NULL,
  user_id         VARCHAR(32) NOT NULL,
  read_at         DATETIME    NOT NULL,
  PRIMARY KEY (announcement_id, user_id),
  KEY idx_ntf_announcement_reads_user_unread (user_id, read_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Welcome 模板表（Phase 4.1 iam.user.created consumer 渲染欢迎站内信）
CREATE TABLE IF NOT EXISTS ntf_templates (
  code         VARCHAR(64)  NOT NULL,
  title        VARCHAR(200) NOT NULL,
  body         MEDIUMTEXT   NOT NULL,
  updated_at   DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (code)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 事件幂等表
CREATE TABLE IF NOT EXISTS ntf_event_idempotency (
  event_id      VARCHAR(64)  NOT NULL,
  event_type    VARCHAR(128) NOT NULL,
  processed_at  DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (event_id),
  KEY idx_ntf_event_idempotency_type_time (event_type, processed_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Outbox 表（Phase 4.3 ntf-outbox-publisher 轮询）
CREATE TABLE IF NOT EXISTS ntf_outbox_events (
  id              BIGINT       NOT NULL AUTO_INCREMENT,
  event_id        CHAR(36)     NOT NULL,
  event_type      VARCHAR(128) NOT NULL,
  routing_key     VARCHAR(128) NOT NULL,
  envelope_json   JSON         NOT NULL,
  created_at      DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  published_at    DATETIME(3)  NULL,
  PRIMARY KEY (id),
  UNIQUE KEY uk_ntf_outbox_event_id (event_id),
  KEY idx_ntf_outbox_unpublished (published_at, id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Welcome 模板 seed
INSERT INTO ntf_templates (code, title, body)
VALUES (
  'welcome',
  'Welcome to the platform, {{username}}!',
  'Hi {{username}}, welcome aboard. You have been added to {{dept_name}} on {{created_at}}. Have a great start!'
)
ON DUPLICATE KEY UPDATE code = code;
