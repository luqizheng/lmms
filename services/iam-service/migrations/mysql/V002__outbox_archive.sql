-- Phase 6-A T4: archive table for iam_outbox_events.
-- Rows >24h old (by published_at) are moved here from the live outbox table.
-- archived_at index supports 30-day retention cleanup.
--
-- Note: not a copy of the full V001 outbox table. Differences:
--   - no uk_iam_outbox_event_id (archive is sink, event_id uniqueness owned by live table)
--   - id is not AUTO_INCREMENT (id comes from the live table on insert)
--   - + archived_at column for retention queries

CREATE TABLE IF NOT EXISTS iam_outbox_events_archive (
    id              BIGINT       NOT NULL,
    event_id        CHAR(36)     NOT NULL,
    event_type      VARCHAR(128) NOT NULL,
    routing_key     VARCHAR(128) NOT NULL,
    envelope_json   JSON         NOT NULL,
    created_at      DATETIME(3)  NOT NULL,
    published_at    DATETIME(3)  NOT NULL,
    archived_at     DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    PRIMARY KEY (id),
    KEY idx_iam_outbox_archive_archived_at (archived_at, id)
);
