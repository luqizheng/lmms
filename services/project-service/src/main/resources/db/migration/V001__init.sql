-- Placeholder baseline for Phase 9 T2 scaffold.
--
-- T3 (V001__init.sql in services/project-service/migrations/mysql) will
-- overwrite this file with the real PMD domain DDL. The placeholder exists
-- only so Flyway can resolve the classpath location at first build/boot.
--
-- DO NOT add business DDL here. Edit services/project-service/migrations/mysql/V001__init.sql
-- instead so the same source of truth feeds both Flyway (mvn package) and
-- the platform check-db-naming.js CI gate.
SELECT 1;
