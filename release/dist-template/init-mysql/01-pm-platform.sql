-- Init schema for PM Platform (apps/pm).
-- MySQL 8.x official image runs files in /docker-entrypoint-initdb.d/ on first boot
-- only when the data directory is empty (i.e. first `up` after `down -v`).
--
-- PM uses a dedicated schema `pm_platform` (tables prefixed pm_/kb_) separate
-- from the Rust services' `superpowers` schema. The app user (`MYSQL_USER` from
-- .env, default `app`) is granted full privileges on pm_platform; Flyway in the
-- PM container will create tables on startup.
CREATE DATABASE IF NOT EXISTS `pm_platform`
    DEFAULT CHARACTER SET utf8mb4
    DEFAULT COLLATE utf8mb4_unicode_ci;

GRANT ALL PRIVILEGES ON `pm_platform`.* TO 'app'@'%';
FLUSH PRIVILEGES;
