-- V011: Bootstrap 初始化并发控制表（WP-BS-01 / WP-BOOTSTRAP-01）。
--
-- 设计基线：docs/superpowers/specs/2026-11-28-bootstrap-concurrency-control-design.md
--
-- iam_bootstrap_init_gate：预先存在的单行 gate 控制行（主键恒为 1），
--   协调 2 秒候选收集窗口的准入与关闭。迁移时预插控制行——不能依赖
--   对空表加锁（窗口关闭事务与候选准入事务必须锁定同一行才能互斥）。
-- iam_bootstrap_init_requests：首轮并发请求的仲裁记录 + 胜出流程的
--   步骤进度（steps JSON）。自增 id 是窗口关闭后的唯一胜出仲裁顺序。
--
-- ⚠️ 迁移执行器先按 ASCII 分号切分、再剥离 -- 注释行，注释里不能出现分号。

CREATE TABLE IF NOT EXISTS iam_bootstrap_init_gate (
    id INT NOT NULL PRIMARY KEY,
    status VARCHAR(16) NOT NULL DEFAULT 'idle',
    window_started_at DATETIME(6) NULL,
    window_ends_at DATETIME(6) NULL,
    winner_request_id BIGINT NULL,
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6))
;

CREATE TABLE IF NOT EXISTS iam_bootstrap_init_requests (
    id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    status VARCHAR(16) NOT NULL DEFAULT 'collecting',
    steps JSON NULL,
    execution_owner VARCHAR(64) NULL,
    execution_epoch BIGINT NOT NULL DEFAULT 0,
    lease_expires_at DATETIME(6) NULL,
    started_at DATETIME(6) NULL,
    finished_at DATETIME(6) NULL,
    last_error VARCHAR(255) NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6))
;

CREATE INDEX idx_iam_bootstrap_init_requests_status ON iam_bootstrap_init_requests (status)
;

-- 预建唯一 gate 控制行（幂等）：业务请求到来前必须已存在。
INSERT IGNORE INTO iam_bootstrap_init_gate (id, status) VALUES (1, 'idle')
;
