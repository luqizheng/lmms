-- Welcome template + event-id idempotency table for Phase 4.1.

CREATE TABLE IF NOT EXISTS ntf_templates (
  code         VARCHAR(64)  NOT NULL,
  title        VARCHAR(200) NOT NULL,
  body         MEDIUMTEXT   NOT NULL,
  updated_at   DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (code)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS ntf_event_idempotency (
  event_id      VARCHAR(64) NOT NULL,
  event_type    VARCHAR(128) NOT NULL,
  processed_at  DATETIME    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (event_id),
  KEY idx_ntf_event_idempotency_type_time (event_type, processed_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

INSERT INTO ntf_templates (code, title, body)
VALUES (
  'welcome',
  'Welcome to the platform, {{username}}!',
  'Hi {{username}}, welcome aboard. You have been added to {{dept_name}} on {{created_at}}. Have a great start!'
)
ON DUPLICATE KEY UPDATE code = code;
