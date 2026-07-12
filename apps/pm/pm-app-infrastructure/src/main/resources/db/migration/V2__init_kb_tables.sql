-- V1__init_kb_tables.sql — knowledge base tables (KB prefix).
-- Table prefix `kb_` registered in scripts/check-db-naming.js.

SET NAMES utf8mb4;

CREATE TABLE kb_spaces (
  id BIGINT NOT NULL AUTO_INCREMENT,
  project_id BIGINT DEFAULT NULL,
  name VARCHAR(128) NOT NULL,
  description TEXT,
  visibility ENUM('private','project','open') NOT NULL DEFAULT 'project',
  created_by BIGINT NOT NULL,
  deleted_at DATETIME DEFAULT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  KEY idx_kb_spaces_project (project_id, deleted_at),
  KEY idx_kb_spaces_visibility (visibility, deleted_at),
  CONSTRAINT fk_kb_spaces_project FOREIGN KEY (project_id) REFERENCES pm_projects(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='Knowledge base spaces';

CREATE TABLE kb_documents (
  id BIGINT NOT NULL AUTO_INCREMENT,
  space_id BIGINT NOT NULL,
  parent_id BIGINT DEFAULT NULL,
  title VARCHAR(256) NOT NULL,
  current_version_id BIGINT DEFAULT NULL,
  doc_type ENUM('markdown','api_spec','design_doc','meeting') NOT NULL DEFAULT 'markdown',
  created_by BIGINT NOT NULL,
  updated_by BIGINT DEFAULT NULL,
  deleted_at DATETIME DEFAULT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  KEY idx_kb_documents_space (space_id, deleted_at),
  KEY idx_kb_documents_parent (parent_id),
  KEY idx_kb_documents_type (doc_type, deleted_at),
  CONSTRAINT fk_kb_documents_space FOREIGN KEY (space_id) REFERENCES kb_spaces(id),
  CONSTRAINT fk_kb_documents_parent FOREIGN KEY (parent_id) REFERENCES kb_documents(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='Knowledge base documents';

CREATE TABLE kb_document_versions (
  id BIGINT NOT NULL AUTO_INCREMENT,
  document_id BIGINT NOT NULL,
  version_no INT NOT NULL,
  title VARCHAR(256) NOT NULL,
  content_md LONGTEXT,
  content_html LONGTEXT,
  editor_id BIGINT NOT NULL,
  edit_summary VARCHAR(255) DEFAULT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uk_kb_doc_version (document_id, version_no),
  KEY idx_kb_doc_versions_doc (document_id, version_no DESC),
  CONSTRAINT fk_kb_doc_versions_doc FOREIGN KEY (document_id) REFERENCES kb_documents(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='Knowledge base document versions';
