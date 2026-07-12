-- V1__init_pm_tables.sql — project management tables (PM prefix).
-- Table prefix `pm_` registered in scripts/check-db-naming.js.
-- All tables use soft-delete (`deleted_at`) + audit columns (`created_at`, `updated_at`).

SET NAMES utf8mb4;

CREATE TABLE pm_projects (
  id BIGINT NOT NULL AUTO_INCREMENT,
  code VARCHAR(32) NOT NULL,
  name VARCHAR(128) NOT NULL,
  description TEXT,
  status ENUM('active','archived') NOT NULL DEFAULT 'active',
  owner_id BIGINT NOT NULL,
  created_by BIGINT NOT NULL,
  deleted_at DATETIME DEFAULT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uk_pm_projects_code (code),
  KEY idx_pm_projects_owner (owner_id),
  KEY idx_pm_projects_status (status, deleted_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='PM projects';

CREATE TABLE pm_requirements (
  id BIGINT NOT NULL AUTO_INCREMENT,
  project_id BIGINT NOT NULL,
  parent_id BIGINT DEFAULT NULL,
  title VARCHAR(256) NOT NULL,
  description TEXT,
  type ENUM('feature','improvement','bug','task') NOT NULL,
  priority ENUM('p0','p1','p2','p3') NOT NULL DEFAULT 'p2',
  status ENUM('draft','open','in_progress','resolved','closed','rejected') NOT NULL DEFAULT 'draft',
  estimate_hours INT DEFAULT NULL,
  assignee_id BIGINT DEFAULT NULL,
  created_by BIGINT NOT NULL,
  deleted_at DATETIME DEFAULT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  KEY idx_pm_req_project (project_id, status, deleted_at),
  KEY idx_pm_req_assignee (assignee_id, status),
  KEY idx_pm_req_parent (parent_id),
  CONSTRAINT fk_pm_req_project FOREIGN KEY (project_id) REFERENCES pm_projects(id),
  CONSTRAINT fk_pm_req_parent FOREIGN KEY (parent_id) REFERENCES pm_requirements(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='PM requirements';

CREATE TABLE pm_user_stories (
  id BIGINT NOT NULL AUTO_INCREMENT,
  requirement_id BIGINT NOT NULL,
  as_a VARCHAR(128) DEFAULT NULL,
  i_want TEXT,
  so_that TEXT,
  story_points TINYINT DEFAULT NULL,
  status ENUM('pending','in_progress','done','rejected') NOT NULL DEFAULT 'pending',
  sort_order INT NOT NULL DEFAULT 0,
  created_by BIGINT NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  KEY idx_pm_stories_req (requirement_id, sort_order),
  CONSTRAINT fk_pm_stories_req FOREIGN KEY (requirement_id) REFERENCES pm_requirements(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='PM user stories';

CREATE TABLE pm_acceptance_criteria (
  id BIGINT NOT NULL AUTO_INCREMENT,
  user_story_id BIGINT NOT NULL,
  given_text TEXT,
  when_text TEXT,
  then_text TEXT,
  checked TINYINT(1) NOT NULL DEFAULT 0,
  sort_order INT NOT NULL DEFAULT 0,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  KEY idx_pm_ac_story (user_story_id, sort_order),
  CONSTRAINT fk_pm_ac_story FOREIGN KEY (user_story_id) REFERENCES pm_user_stories(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='PM acceptance criteria';
