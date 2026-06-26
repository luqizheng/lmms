-- Align user_id columns with iam-service's nanoid (VARCHAR) instead of BIGINT
-- This migration is idempotent and safe to run multiple times

-- Alter ntf_announcement_deliveries.user_id from BIGINT to VARCHAR(32)
ALTER TABLE ntf_announcement_deliveries
  MODIFY COLUMN user_id VARCHAR(32) NOT NULL;

-- Alter ntf_announcement_reads.user_id from BIGINT to VARCHAR(32)
ALTER TABLE ntf_announcement_reads
  MODIFY COLUMN user_id VARCHAR(32) NOT NULL;

-- Alter ntf_announcements.created_by from BIGINT to VARCHAR(32)
ALTER TABLE ntf_announcements
  MODIFY COLUMN created_by VARCHAR(32) NOT NULL;

-- Alter ntf_announcement_recipients.scope_id from BIGINT to VARCHAR(32)
ALTER TABLE ntf_announcement_recipients
  MODIFY COLUMN scope_id VARCHAR(32) NOT NULL;