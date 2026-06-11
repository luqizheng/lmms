-- 1) org_units
CREATE TABLE IF NOT EXISTS org_units (
  id          BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  code        VARCHAR(64) NOT NULL UNIQUE,
  name        VARCHAR(128) NOT NULL,
  parent_id   BIGINT UNSIGNED NULL,
  sort        INT NOT NULL DEFAULT 0,
  status      TINYINT NOT NULL DEFAULT 1,
  created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  KEY idx_org_parent (parent_id),
  CONSTRAINT fk_org_parent FOREIGN KEY (parent_id) REFERENCES org_units(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- 2) positions
CREATE TABLE IF NOT EXISTS positions (
  id          BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  code        VARCHAR(64) NOT NULL UNIQUE,
  name        VARCHAR(128) NOT NULL,
  org_unit_id BIGINT UNSIGNED NOT NULL,
  level       TINYINT NOT NULL DEFAULT 0,
  status      TINYINT NOT NULL DEFAULT 1,
  created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  KEY idx_pos_org (org_unit_id),
  CONSTRAINT fk_pos_org FOREIGN KEY (org_unit_id) REFERENCES org_units(id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- 3) position_roles
CREATE TABLE IF NOT EXISTS position_roles (
  position_id BIGINT UNSIGNED NOT NULL,
  role_id     BIGINT UNSIGNED NOT NULL,
  PRIMARY KEY (position_id, role_id),
  KEY idx_pr_role (role_id),
  CONSTRAINT fk_pr_pos FOREIGN KEY (position_id) REFERENCES positions(id) ON DELETE CASCADE,
  CONSTRAINT fk_pr_role FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- 4) user_positions
CREATE TABLE IF NOT EXISTS user_positions (
  user_id     BIGINT UNSIGNED NOT NULL,
  position_id BIGINT UNSIGNED NOT NULL,
  is_primary  TINYINT NOT NULL DEFAULT 0,
  assigned_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (user_id, position_id),
  KEY idx_up_pos (position_id),
  KEY idx_up_primary (user_id, is_primary),
  CONSTRAINT fk_up_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_up_pos  FOREIGN KEY (position_id) REFERENCES positions(id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- 5) 迁移历史 dept/title
INSERT IGNORE INTO org_units (code, name)
SELECT DISTINCT
  CONCAT('legacy-', JSON_UNQUOTE(JSON_EXTRACT(profile_json, '$.dept'))) AS code,
  JSON_UNQUOTE(JSON_EXTRACT(profile_json, '$.dept')) AS name
FROM users
WHERE JSON_EXTRACT(profile_json, '$.dept') IS NOT NULL
  AND JSON_UNQUOTE(JSON_EXTRACT(profile_json, '$.dept')) <> '';

INSERT IGNORE INTO positions (code, name, org_unit_id)
SELECT DISTINCT
  CONCAT('legacy-pos-', u.id) AS code,
  JSON_UNQUOTE(JSON_EXTRACT(u.profile_json, '$.title')) AS name,
  ou.id AS org_unit_id
FROM users u
INNER JOIN org_units ou
  ON ou.code = CONCAT('legacy-', JSON_UNQUOTE(JSON_EXTRACT(u.profile_json, '$.dept')))
WHERE JSON_EXTRACT(u.profile_json, '$.title') IS NOT NULL
  AND JSON_UNQUOTE(JSON_EXTRACT(u.profile_json, '$.title')) <> '';

INSERT IGNORE INTO user_positions (user_id, position_id, is_primary)
SELECT u.id, p.id, 1
FROM users u
INNER JOIN positions p
  ON p.code = CONCAT('legacy-pos-', u.id)
WHERE JSON_EXTRACT(u.profile_json, '$.title') IS NOT NULL;
