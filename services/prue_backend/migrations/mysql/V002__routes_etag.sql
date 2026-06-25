ALTER TABLE menu_configs ADD COLUMN routes_etag CHAR(64) NOT NULL DEFAULT '' AFTER routes_json;
UPDATE menu_configs SET routes_etag = SHA2(CAST(routes_json AS CHAR), 256) WHERE routes_etag = '';
