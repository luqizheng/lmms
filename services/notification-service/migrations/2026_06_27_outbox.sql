-- Outbox table for notification-service domain events (Phase 4.3).
-- notification-service writes here in the same transaction as the business row;
-- the ntf-outbox-publisher binary polls and publishes to RabbitMQ.
-- Columns MUST match common_event::MySqlOutboxRepo's SQL exactly.
CREATE TABLE IF NOT EXISTS ntf_outbox_events (
    id              BIGINT       NOT NULL AUTO_INCREMENT,
    event_id        CHAR(36)     NOT NULL,
    event_type      VARCHAR(128) NOT NULL,
    routing_key     VARCHAR(128) NOT NULL,
    envelope_json   JSON         NOT NULL,
    created_at      DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    published_at    DATETIME(3)  NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uk_ntf_outbox_event_id (event_id),
    KEY idx_ntf_outbox_unpublished (published_at, id)
);
