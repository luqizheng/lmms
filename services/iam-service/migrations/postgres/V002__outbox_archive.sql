-- PG 版：与 mysql V002 语义对齐（TIMESTAMPTZ/BIGINT 有符号/JSONB/独立索引语句）
-- iam-service PostgreSQL V002 — 与 mysql/V002__outbox_archive.sql 版本号与语义对齐。
--
-- Phase 6-A T4: archive table for iam_outbox_events.
-- Rows >24h old (by published_at) are moved here from the live outbox table.
-- archived_at index supports 30-day retention cleanup.
--
-- Note: not a copy of the full V001 outbox table. Differences:
--   - no uk_iam_outbox_event_id (archive is sink, event_id uniqueness owned by live table)
--   - id is not IDENTITY (id comes from the live table on insert)
--   - + archived_at column for retention queries
--
-- 转换规则：
--   JSON        -> JSONB（libs/common-event/src/outbox_archive.rs 的 PG 臂按
--                  id / event_id / event_type / routing_key / envelope_json /
--                  created_at / published_at / archived_at 读写，列名列序一致）
--   DATETIME(3) -> TIMESTAMPTZ(3)（DEFAULT CURRENT_TIMESTAMP(3) 语义原样保留）
--   BIGINT      -> BIGINT（归档行 id 来自 live 表，无自增）
--   表内 KEY    -> 独立 CREATE INDEX IF NOT EXISTS
--
-- ⚠️ 注释里不得出现 ASCII 分号（执行器按分号切分）。

CREATE TABLE IF NOT EXISTS iam_outbox_events_archive (
    id              BIGINT       NOT NULL,
    event_id        CHAR(36)     NOT NULL,
    event_type      VARCHAR(128) NOT NULL,
    routing_key     VARCHAR(128) NOT NULL,
    envelope_json   JSONB        NOT NULL,
    created_at      TIMESTAMPTZ(3) NOT NULL,
    published_at    TIMESTAMPTZ(3) NOT NULL,
    archived_at     TIMESTAMPTZ(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_iam_outbox_archive_archived_at
    ON iam_outbox_events_archive (archived_at, id);
