-- V4: pm_sprints + add sprint_id to pm_tasks. Phase D5 sprint management:
-- sprints group tasks into time-boxed iterations (planning/active/completed).
-- We add a nullable sprint_id FK on pm_tasks rather than a join table -
-- each task can be in at most one sprint, which is the common PM convention.
CREATE TABLE pm_sprints (
  id BIGINT NOT NULL AUTO_INCREMENT,
  project_id BIGINT NOT NULL,
  name VARCHAR(128) NOT NULL,
  goal TEXT,
  start_date DATE DEFAULT NULL,
  end_date DATE DEFAULT NULL,
  status ENUM('planning','active','completed') NOT NULL DEFAULT 'planning',
  created_by BIGINT NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  deleted_at DATETIME DEFAULT NULL,
  PRIMARY KEY (id),
  KEY idx_pm_sprints_project (project_id, deleted_at, status, start_date),
  KEY idx_pm_sprints_status (status, deleted_at, end_date)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
  COMMENT='PM sprints (Phase D5 sprint management)';

-- Each task belongs to at most one sprint; null = backlog.
ALTER TABLE pm_tasks
  ADD COLUMN sprint_id BIGINT DEFAULT NULL AFTER project_id,
  ADD KEY idx_pm_tasks_sprint (sprint_id, deleted_at);
