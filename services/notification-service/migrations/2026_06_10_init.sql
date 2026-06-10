CREATE DATABASE IF NOT EXISTS notification
  CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

USE notification;

CREATE TABLE IF NOT EXISTS announcements (
  id              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  title           VARCHAR(200)    NOT NULL,
  body            MEDIUMTEXT      NOT NULL,
  publish_at      DATETIME        NOT NULL,
  expire_at       DATETIME        NULL,
  pinned          TINYINT(1)      NOT NULL DEFAULT 0,
  popover         TINYINT(1)      NOT NULL DEFAULT 0,
  status          ENUM('draft','scheduled','published','revoked') NOT NULL,
  created_by      BIGINT          NOT NULL,
  created_at      DATETIME        NOT NULL,
  PRIMARY KEY (id),
  KEY idx_status_publish (status, publish_at)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS announcement_recipients (
  announcement_id BIGINT NOT NULL,
  scope_type      ENUM('user','role','dept') NOT NULL,
  scope_id        BIGINT NOT NULL,
  PRIMARY KEY (announcement_id, scope_type, scope_id)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS announcement_deliveries (
  announcement_id BIGINT  NOT NULL,
  user_id         BIGINT  NOT NULL,
  delivered_at    DATETIME NOT NULL,
  PRIMARY KEY (announcement_id, user_id),
  KEY idx_user (user_id, announcement_id)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS announcement_reads (
  announcement_id BIGINT  NOT NULL,
  user_id         BIGINT  NOT NULL,
  read_at         DATETIME NOT NULL,
  PRIMARY KEY (announcement_id, user_id),
  KEY idx_user_unread (user_id, read_at)
) ENGINE=InnoDB;
