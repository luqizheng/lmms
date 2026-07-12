-- V3: pm_tasks. Phase D4 task management: standalone tasks (optionally linked
-- to a project / requirement) with assignee + priority + status + due date.
-- Soft-delete via deleted_at; soft_deleted rows are filtered by MyBatis-Plus
-- @TableLogic on the entity (or QueryWrapper isNull).
CREATE TABLE pm_tasks (
  id BIGINT NOT NULL AUTO_INCREMENT,
  project_id BIGINT NOT NULL,
  requirement_id BIGINT DEFAULT NULL,
  title VARCHAR(256) NOT NULL,
  description TEXT,
  status ENUM('todo','in_progress','done') NOT NULL DEFAULT 'todo',
  priority ENUM('low','medium','high') NOT NULL DEFAULT 'medium',
  assignee_id BIGINT DEFAULT NULL,
  due_date DATE DEFAULT NULL,
  sort_order INT NOT NULL DEFAULT 0,
  created_by BIGINT NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  deleted_at DATETIME DEFAULT NULL,
  PRIMARY KEY (id),
  KEY idx_pm_tasks_project (project_id, deleted_at, sort_order),
  KEY idx_pm_tasks_assignee (assignee_id, deleted_at),
  KEY idx_pm_tasks_requirement (requirement_id, deleted_at),
  KEY idx_pm_tasks_due (due_date, deleted_at, status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
  COMMENT='PM tasks (Phase D4 task management)';
