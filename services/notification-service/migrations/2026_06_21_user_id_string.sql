-- Align user_id columns with iam-service's nanoid (VARCHAR) instead of BIGINT
-- This migration is idempotent and safe to run multiple times

-- Alter announcement_deliveries.user_id from BIGINT to VARCHAR(32)
ALTER TABLE announcement_deliveries
  MODIFY COLUMN user_id VARCHAR(32) NOT NULL;

-- Alter announcement_reads.user_id from BIGINT to VARCHAR(32)
ALTER TABLE announcement_reads
  MODIFY COLUMN user_id VARCHAR(32) NOT NULL;

-- Alter announcements.created_by from BIGINT to VARCHAR(32)
ALTER TABLE announcements
  MODIFY COLUMN created_by VARCHAR(32) NOT NULL;

-- Alter announcement_recipients.scope_id from BIGINT to VARCHAR(32)
ALTER TABLE announcement_recipients
  MODIFY COLUMN scope_id VARCHAR(32) NOT NULL;