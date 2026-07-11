-- Phase 9 PMD service initial schema (MySQL).
-- Forward-only; written in line with libs/common-event envelope columns
-- (event_id, event_type, routing_key, envelope_json, trace_id, tenant_id,
-- producer, published_at, attempts, last_error) so the Rust
-- pmd-outbox-publisher can poll rows written by Java OutboxWriter.
--
-- Idempotent: every CREATE uses IF NOT EXISTS so a re-apply is safe.

CREATE TABLE IF NOT EXISTS pmd_projects (
    id            BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    tenant_id     BIGINT UNSIGNED NOT NULL,
    code          VARCHAR(64)  NOT NULL,
    name          VARCHAR(255) NOT NULL,
    description   TEXT NULL,
    status        ENUM('active','archived','cancelled') NOT NULL DEFAULT 'active',
    owner_id      VARCHAR(64) NOT NULL,
    start_date    DATE NULL,
    end_date      DATE NULL,
    actor_id      VARCHAR(64) NULL,
    created_at    DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    updated_at    DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3),
    PRIMARY KEY (id),
    UNIQUE KEY uk_pmd_projects_code (code),
    KEY idx_pmd_projects_tenant (tenant_id),
    KEY idx_pmd_projects_owner (owner_id),
    KEY idx_pmd_projects_status (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS pmd_milestones (
    id            BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    tenant_id     BIGINT UNSIGNED NOT NULL,
    project_id    BIGINT UNSIGNED NOT NULL,
    code          VARCHAR(64)  NOT NULL,
    name          VARCHAR(255) NOT NULL,
    due_date      DATE NULL,
    status        ENUM('pending','reached','missed','cancelled') NOT NULL DEFAULT 'pending',
    actor_id      VARCHAR(64) NULL,
    created_at    DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    updated_at    DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3),
    PRIMARY KEY (id),
    UNIQUE KEY uk_pmd_milestones_project_code (project_id, code),
    KEY idx_pmd_milestones_project (project_id),
    KEY idx_pmd_milestones_tenant (tenant_id),
    KEY idx_pmd_milestones_status (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS pmd_tasks (
    id              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    tenant_id       BIGINT UNSIGNED NOT NULL,
    project_id      BIGINT UNSIGNED NOT NULL,
    milestone_id    BIGINT UNSIGNED NULL,
    sprint_id       BIGINT UNSIGNED NULL,
    code            VARCHAR(64)  NOT NULL,
    title           VARCHAR(255) NOT NULL,
    description     TEXT NULL,
    assignee_id     VARCHAR(64) NULL,
    status          ENUM('backlog','todo','in_progress','blocked','review','done','cancelled') NOT NULL DEFAULT 'backlog',
    estimate_hours  DECIMAL(6,2) NULL,
    actor_id        VARCHAR(64) NULL,
    created_at      DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    updated_at      DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3),
    PRIMARY KEY (id),
    UNIQUE KEY uk_pmd_tasks_project_code (project_id, code),
    KEY idx_pmd_tasks_project (project_id),
    KEY idx_pmd_tasks_milestone (milestone_id),
    KEY idx_pmd_tasks_assignee (assignee_id),
    KEY idx_pmd_tasks_tenant (tenant_id),
    KEY idx_pmd_tasks_status (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS pmd_sprint_mappings (
    id            BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    tenant_id     BIGINT UNSIGNED NOT NULL,
    project_id    BIGINT UNSIGNED NOT NULL,
    sprint_name   VARCHAR(64) NOT NULL,
    starts_on     DATE NOT NULL,
    ends_on       DATE NOT NULL,
    actor_id      VARCHAR(64) NULL,
    created_at    DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    updated_at    DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3),
    PRIMARY KEY (id),
    UNIQUE KEY uk_pmd_sprint_project_name (project_id, sprint_name),
    KEY idx_pmd_sprint_project (project_id),
    KEY idx_pmd_sprint_tenant (tenant_id),
    KEY idx_pmd_sprint_window (starts_on, ends_on)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Outbox events table; column names mirror libs/common-event::outbox::MySqlOutboxRepo::enqueue.
-- The Rust pmd-outbox-publisher polls this table with FOR UPDATE SKIP LOCKED.
CREATE TABLE IF NOT EXISTS pmd_outbox_events (
    id              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    event_id        CHAR(36)     NOT NULL,
    event_type      VARCHAR(128) NOT NULL,
    routing_key     VARCHAR(128) NOT NULL,
    envelope_json   JSON         NOT NULL,
    trace_id        VARCHAR(64)  NULL,
    tenant_id       BIGINT UNSIGNED NULL,
    producer        VARCHAR(64)  NOT NULL DEFAULT 'project-service',
    created_at      DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    published_at    DATETIME(3)  NULL,
    attempts        INT          NOT NULL DEFAULT 0,
    last_error      TEXT         NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uk_pmd_outbox_event_id (event_id),
    KEY idx_pmd_outbox_published_id (published_at, id),
    KEY idx_pmd_outbox_routing (routing_key),
    KEY idx_pmd_outbox_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS pmd_llm_refinement_log (
    id              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    tenant_id       BIGINT UNSIGNED NULL,
    project_id      BIGINT UNSIGNED NULL,
    task_id         BIGINT UNSIGNED NULL,
    caller_user_id  VARCHAR(64) NOT NULL,
    request         TEXT NOT NULL,
    response        TEXT NULL,
    created_at      DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    PRIMARY KEY (id),
    KEY idx_pmd_llm_log_caller (caller_user_id),
    KEY idx_pmd_llm_log_tenant (tenant_id),
    KEY idx_pmd_llm_log_project (project_id),
    KEY idx_pmd_llm_log_task (task_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
