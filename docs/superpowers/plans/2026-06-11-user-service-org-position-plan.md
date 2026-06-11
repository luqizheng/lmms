# User Service 组织单元与职位 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `user-service` 新增组织单元（OrgUnit）与职位（Position）实体；职位绑定角色，用户挂职位即获角色授权。

**Architecture:** 四张新表（`org_units` / `positions` / `position_roles` / `user_positions`），三层新路由（org / position / user_position），改造 `UserDTO` 与 `fetch_user_with_roles` 把职位角色并入授权集合，一次性迁移脚本把 `user.profile_json.dept/title` 转为实体。

**Tech Stack:** Rust 1.x, axum 0.7, sqlx 0.8 (mysql), serde, MySQL 8.0 (CTE recursive)。

**Spec:** [`docs/superpowers/specs/2026-06-11-user-service-org-position-design.md`](../specs/2026-06-11-user-service-org-position-design.md)

**前置条件:**
- 真实 PowerShell（vcvarsall x64）跑 cargo，沙箱只读
- MySQL 8.0 实例（与 user-service 现有开发库一致）

**Commit 前缀:** `feat(user-service):` / `fix(user-service):`

---

## File Structure

**Create:**
```
services/user-service/src/models/org_unit.rs          # OrgUnitRow + DTO
services/user-service/src/models/position.rs          # PositionRow + DTO
services/user-service/src/models/user_position.rs     # UserPositionRow + DTO
services/user-service/src/service/org_svc.rs           # 组织 CRUD + 树查询
services/user-service/src/service/position_svc.rs      # 职位 CRUD + 绑角色
services/user-service/src/service/user_position_svc.rs # 用户职位绑定 + 主职唯一
services/user-service/src/routes/org_units.rs          # 组织路由
services/user-service/src/routes/positions.rs          # 职位路由
services/user-service/src/routes/user_positions.rs     # 用户职位路由
services/user-service/tests/org_position_test.rs       # 组织/职位/绑定测试
services/user-service/tests/authz_combination_test.rs  # 授权合并测试
scripts/migrations/2026_06_11_org_position_init.sql    # 迁移脚本
```

**Modify:**
```
services/user-service/src/models/mod.rs                # 加三个子模块
services/user-service/src/models/user.rs               # UserDTO 加 positions
services/user-service/src/service/mod.rs               # 加三个 svc 子模块
services/user-service/src/service/user_svc.rs          # 合并职位角色
services/user-service/src/routes/mod.rs                # 加三个 routes 子模块
services/user-service/src/routes/auth.rs               # 挂载新路由
services/user-service/src/main.rs                      # 同上
services/user-service/src/dto.rs                       # 新增请求 DTO
```

---

## Task 1: 数据库迁移脚本

**Files:**
- Create: `scripts/migrations/2026_06_11_org_position_init.sql`

- [ ] **Step 1: 写四张新表 DDL**

`scripts/migrations/2026_06_11_org_position_init.sql`：
```sql
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
-- 一次性把 users.profile_json 的 dept/title 建为 org_units + positions + user_positions
-- 重复执行幂等
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
```

幂等：全部 `CREATE TABLE IF NOT EXISTS` + `INSERT IGNORE`。

- [ ] **Step 2: 在测试容器里执行验证**

在真实 PowerShell 跑：
```powershell
docker exec -i coder-mysql mysql -uroot -proot user_service < scripts/migrations/2026_06_11_org_position_init.sql
```
预期：无错误。`SHOW TABLES LIKE 'org%' / 'position%' / 'user_pos%'` 返回三张表。

- [ ] **Step 3: 提交**

```bash
git add scripts/migrations/2026_06_11_org_position_init.sql
git commit -m "feat(user-service): add org_units / positions / position_roles / user_position..."
```

---

## Task 2: OrgUnit 模型与 DTO

**Files:**
- Create: `services/user-service/src/models/org_unit.rs`
- Modify: `services/user-service/src/models/mod.rs`

- [ ] **Step 1: 创建 OrgUnitRow + OrgUnitDTO**

`services/user-service/src/models/org_unit.rs`：
```rust
use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrgUnitRow {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub parent_id: Option<u64>,
    pub sort: i32,
    pub status: i8,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrgUnitDTO {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub parent_id: Option<u64>,
    pub sort: i32,
    pub status: i8,
    pub created_at: String,
    pub updated_at: String,
}

impl OrgUnitRow {
    pub fn into_dto(self) -> OrgUnitDTO {
        OrgUnitDTO {
            id: self.id,
            code: self.code,
            name: self.name,
            parent_id: self.parent_id,
            sort: self.sort,
            status: self.status,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
```

- [ ] **Step 2: 注册子模块**

`services/user-service/src/models/mod.rs`：
```rust
pub mod login_log;
pub mod org_unit;
pub mod permission;
pub mod position;
pub mod role;
pub mod user;
pub mod user_position;
```

- [ ] **Step 3: 编译验证**

```powershell
cd services/user-service; cargo check
```
预期：`error[E0432]: unresolved import 'crate::models::position'`
（这是预期的，下一 task 解决）

- [ ] **Step 4: 提交**

```bash
git add services/user-service/src/models/org_unit.rs services/user-service/src/models/mod.rs
git commit -m "feat(user-service): add org_unit model"
```

---

## Task 3: Position 模型与 DTO

**Files:**
- Create: `services/user-service/src/models/position.rs`

- [ ] **Step 1: 创建 PositionRow + PositionDTO**

`services/user-service/src/models/position.rs`：
```rust
use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PositionRow {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub org_unit_id: u64,
    pub level: i8,
    pub status: i8,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PositionDTO {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub org_unit_id: u64,
    pub org_unit_code: String,
    pub level: i8,
    pub status: i8,
    pub role_codes: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl PositionRow {
    pub fn into_dto(self, org_unit_code: String, role_codes: Vec<String>) -> PositionDTO {
        PositionDTO {
            id: self.id,
            code: self.code,
            name: self.name,
            org_unit_id: self.org_unit_id,
            org_unit_code,
            level: self.level,
            status: self.status,
            role_codes,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
```

- [ ] **Step 2: 编译验证**

```powershell
cd services/user-service; cargo check
```
预期：仅 `user_position` 模块未找到的 error。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/models/position.rs
git commit -m "feat(user-service): add position model"
```

---

## Task 4: UserPosition 模型与 DTO

**Files:**
- Create: `services/user-service/src/models/user_position.rs`

- [ ] **Step 1: 创建 UserPositionRow + DTO**

`services/user-service/src/models/user_position.rs`：
```rust
use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserPositionRow {
    pub user_id: u64,
    pub position_id: u64,
    pub is_primary: i8,
    pub assigned_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserPositionDTO {
    pub position_id: u64,
    pub position_code: String,
    pub position_name: String,
    pub org_unit_id: u64,
    pub org_unit_name: String,
    pub is_primary: bool,
    pub assigned_at: String,
}
```

- [ ] **Step 2: 编译验证**

```powershell
cd services/user-service; cargo check
```
预期：编译通过。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/models/user_position.rs
git commit -m "feat(user-service): add user_position model"
```

---

## Task 5: org_svc 服务层

**Files:**
- Create: `services/user-service/src/service/org_svc.rs`
- Modify: `services/user-service/src/service/mod.rs`

- [ ] **Step 1: 实现 CRUD + 树查询**

`services/user-service/src/service/org_svc.rs`：
```rust
use crate::db::DbPool;
use crate::error::AppError;
use crate::models::org_unit::{OrgUnitDTO, OrgUnitRow};

pub async fn list_org_units(
    pool: &DbPool,
    parent_id: Option<u64>,
) -> Result<Vec<OrgUnitDTO>, AppError> {
    let DbPool::Mysql(p) = pool;
    let rows: Vec<OrgUnitRow> = if let Some(pid) = parent_id {
        sqlx::query_as(
            "SELECT id, code, name, parent_id, sort, status, created_at, updated_at \
             FROM org_units WHERE parent_id <=> ? ORDER BY sort, id",
        )
        .bind(pid)
        .fetch_all(p)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, code, name, parent_id, sort, status, created_at, updated_at \
             FROM org_units ORDER BY sort, id",
        )
        .fetch_all(p)
        .await?
    };
    Ok(rows.into_iter().map(OrgUnitRow::into_dto).collect())
}

pub async fn create_org_unit(
    pool: &DbPool,
    code: &str,
    name: &str,
    parent_id: Option<u64>,
    sort: i32,
) -> Result<OrgUnitDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    let res = sqlx::query(
        "INSERT INTO org_units (code, name, parent_id, sort) VALUES (?, ?, ?, ?)",
    )
    .bind(code)
    .bind(name)
    .bind(parent_id)
    .bind(sort)
    .execute(p)
    .await?;
    let id = res.last_insert_id();
    get_org_unit(pool, id).await
}

pub async fn get_org_unit(pool: &DbPool, id: u64) -> Result<OrgUnitDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    let row: OrgUnitRow = sqlx::query_as(
        "SELECT id, code, name, parent_id, sort, status, created_at, updated_at \
         FROM org_units WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(p)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("org_unit id={id}")))?;
    Ok(row.into_dto())
}

pub async fn update_org_unit(
    pool: &DbPool,
    id: u64,
    name: &str,
    parent_id: Option<u64>,
    sort: i32,
    status: i8,
) -> Result<OrgUnitDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    sqlx::query(
        "UPDATE org_units SET name = ?, parent_id = ?, sort = ?, status = ?, updated_at = NOW() \
         WHERE id = ?",
    )
    .bind(name)
    .bind(parent_id)
    .bind(sort)
    .bind(status)
    .bind(id)
    .execute(p)
    .await?;
    get_org_unit(pool, id).await
}

pub async fn delete_org_unit(pool: &DbPool, id: u64) -> Result<(), AppError> {
    let DbPool::Mysql(p) = pool;
    let child: Option<(u64,)> = sqlx::query_as("SELECT id FROM org_units WHERE parent_id = ? LIMIT 1")
        .bind(id)
        .fetch_optional(p)
        .await?;
    if child.is_some() {
        return Err(AppError::Validation("org_unit has children".into()));
    }
    let pos: Option<(u64,)> = sqlx::query_as("SELECT id FROM positions WHERE org_unit_id = ? LIMIT 1")
        .bind(id)
        .fetch_optional(p)
        .await?;
    if pos.is_some() {
        return Err(AppError::Validation("org_unit has positions".into()));
    }
    sqlx::query("DELETE FROM org_units WHERE id = ?")
        .bind(id)
        .execute(p)
        .await?;
    Ok(())
}

pub async fn get_org_tree(pool: &DbPool, root_id: u64) -> Result<Vec<OrgUnitDTO>, AppError> {
    let DbPool::Mysql(p) = pool;
    let rows: Vec<OrgUnitRow> = sqlx::query_as(
        "WITH RECURSIVE tree AS ( \
            SELECT id, code, name, parent_id, sort, status, created_at, updated_at \
            FROM org_units WHERE id = ? \
            UNION ALL \
            SELECT o.id, o.code, o.name, o.parent_id, o.sort, o.status, o.created_at, o.updated_at \
            FROM org_units o INNER JOIN tree t ON o.parent_id = t.id \
         ) SELECT * FROM tree ORDER BY sort, id",
    )
    .bind(root_id)
    .fetch_all(p)
    .await?;
    Ok(rows.into_iter().map(OrgUnitRow::into_dto).collect())
}
```

- [ ] **Step 2: 注册子模块**

`services/user-service/src/service/mod.rs`：
```rust
pub mod auth_svc;
pub mod claim_svc;
pub mod org_svc;
pub mod position_svc;
pub mod role_svc;
pub mod user_position_svc;
pub mod user_svc;
```

- [ ] **Step 3: 编译验证**

```powershell
cd services/user-service; cargo check
```
预期：仅 `position_svc` / `user_position_svc` 模块未找到 error。

- [ ] **Step 4: 提交**

```bash
git add services/user-service/src/service/org_svc.rs services/user-service/src/service/mod.rs
git commit -m "feat(user-service): add org_svc CRUD + recursive tree query"
```

---

## Task 6: position_svc 服务层

**Files:**
- Create: `services/user-service/src/service/position_svc.rs`

- [ ] **Step 1: 实现 CRUD + 角色绑定**

`services/user-service/src/service/position_svc.rs`：
```rust
use crate::db::DbPool;
use crate::error::AppError;
use crate::models::position::{PositionDTO, PositionRow};

pub async fn list_positions(
    pool: &DbPool,
    org_unit_id: Option<u64>,
) -> Result<Vec<PositionDTO>, AppError> {
    let DbPool::Mysql(p) = pool;
    let rows: Vec<PositionRow> = if let Some(oid) = org_unit_id {
        sqlx::query_as(
            "SELECT id, code, name, org_unit_id, level, status, created_at, updated_at \
             FROM positions WHERE org_unit_id = ? ORDER BY id",
        )
        .bind(oid)
        .fetch_all(p)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, code, name, org_unit_id, level, status, created_at, updated_at \
             FROM positions ORDER BY id",
        )
        .fetch_all(p)
        .await?
    };
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let org_code: (String,) = sqlx::query_as("SELECT code FROM org_units WHERE id = ?")
            .bind(r.org_unit_id)
            .fetch_one(p)
            .await?;
        let role_codes = fetch_position_role_codes(pool, r.id).await?;
        out.push(r.into_dto(org_code.0, role_codes));
    }
    Ok(out)
}

pub async fn create_position(
    pool: &DbPool,
    code: &str,
    name: &str,
    org_unit_id: u64,
    level: i8,
) -> Result<PositionDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    let res = sqlx::query(
        "INSERT INTO positions (code, name, org_unit_id, level) VALUES (?, ?, ?, ?)",
    )
    .bind(code)
    .bind(name)
    .bind(org_unit_id)
    .bind(level)
    .execute(p)
    .await?;
    get_position(pool, res.last_insert_id()).await
}

pub async fn get_position(pool: &DbPool, id: u64) -> Result<PositionDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    let row: PositionRow = sqlx::query_as(
        "SELECT id, code, name, org_unit_id, level, status, created_at, updated_at \
         FROM positions WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(p)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("position id={id}")))?;
    let org: (String,) = sqlx::query_as("SELECT code FROM org_units WHERE id = ?")
        .bind(row.org_unit_id)
        .fetch_one(p)
        .await?;
    let role_codes = fetch_position_role_codes(pool, id).await?;
    Ok(row.into_dto(org.0, role_codes))
}

pub async fn update_position(
    pool: &DbPool,
    id: u64,
    name: &str,
    org_unit_id: u64,
    level: i8,
    status: i8,
) -> Result<PositionDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    sqlx::query(
        "UPDATE positions SET name = ?, org_unit_id = ?, level = ?, status = ?, updated_at = NOW() \
         WHERE id = ?",
    )
    .bind(name)
    .bind(org_unit_id)
    .bind(level)
    .bind(status)
    .bind(id)
    .execute(p)
    .await?;
    get_position(pool, id).await
}

pub async fn delete_position(pool: &DbPool, id: u64) -> Result<(), AppError> {
    let DbPool::Mysql(p) = pool;
    let used: Option<(u64,)> = sqlx::query_as(
        "SELECT user_id FROM user_positions WHERE position_id = ? LIMIT 1",
    )
    .bind(id)
    .fetch_optional(p)
    .await?;
    if used.is_some() {
        return Err(AppError::Validation("position is assigned to users".into()));
    }
    sqlx::query("DELETE FROM positions WHERE id = ?")
        .bind(id)
        .execute(p)
        .await?;
    Ok(())
}

pub async fn assign_position_roles(
    pool: &DbPool,
    position_id: u64,
    role_ids: &[u64],
) -> Result<(), AppError> {
    let DbPool::Mysql(p) = pool;
    let mut tx = p.begin().await?;
    sqlx::query("DELETE FROM position_roles WHERE position_id = ?")
        .bind(position_id)
        .execute(&mut *tx)
        .await?;
    for rid in role_ids {
        sqlx::query("INSERT IGNORE INTO position_roles (position_id, role_id) VALUES (?, ?)")
            .bind(position_id)
            .bind(rid)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn fetch_position_role_codes(
    pool: &DbPool,
    position_id: u64,
) -> Result<Vec<String>, AppError> {
    let DbPool::Mysql(p) = pool;
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT r.code FROM roles r INNER JOIN position_roles pr ON pr.role_id = r.id \
         WHERE pr.position_id = ?",
    )
    .bind(position_id)
    .fetch_all(p)
    .await?;
    Ok(rows.into_iter().map(|(c,)| c).collect())
}
```

- [ ] **Step 2: 编译验证**

```powershell
cd services/user-service; cargo check
```
预期：仅 `user_position_svc` 模块未找到 error。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/service/position_svc.rs
git commit -m "feat(user-service): add position_svc CRUD + role binding"
```

---

## Task 7: user_position_svc 服务层

**Files:**
- Create: `services/user-service/src/service/user_position_svc.rs`

- [ ] **Step 1: 实现绑定 + 主职唯一事务**

`services/user-service/src/service/user_position_svc.rs`：
```rust
use crate::db::DbPool;
use crate::error::AppError;
use crate::models::user_position::UserPositionDTO;

#[derive(Debug, Clone)]
pub struct PositionAssignItem {
    pub position_id: u64,
    pub is_primary: bool,
}

pub async fn list_user_positions(
    pool: &DbPool,
    user_id: u64,
) -> Result<Vec<UserPositionDTO>, AppError> {
    let DbPool::Mysql(p) = pool;
    let rows: Vec<(u64, String, String, u64, String, i8, String)> = sqlx::query_as(
        "SELECT up.position_id, p.code, p.name, ou.id, ou.name, up.is_primary, up.assigned_at \
         FROM user_positions up \
         INNER JOIN positions p ON p.id = up.position_id \
         INNER JOIN org_units ou ON ou.id = p.org_unit_id \
         WHERE up.user_id = ? ORDER BY up.is_primary DESC, up.assigned_at",
    )
    .bind(user_id)
    .fetch_all(p)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(pid, pcode, pname, ouid, ourname, prim, assigned_at)| UserPositionDTO {
            position_id: pid,
            position_code: pcode,
            position_name: pname,
            org_unit_id: ouid,
            org_unit_name: ourname,
            is_primary: prim != 0,
            assigned_at,
        })
        .collect())
}

pub async fn replace_user_positions(
    pool: &DbPool,
    user_id: u64,
    items: &[PositionAssignItem],
) -> Result<Vec<UserPositionDTO>, AppError> {
    let DbPool::Mysql(p) = pool;
    let mut tx = p.begin().await?;
    sqlx::query("DELETE FROM user_positions WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let primary_count = items.iter().filter(|i| i.is_primary).count();
    if primary_count > 1 {
        return Err(AppError::Validation("only one primary position allowed".into()));
    }
    for it in items {
        sqlx::query(
            "INSERT INTO user_positions (user_id, position_id, is_primary) VALUES (?, ?, ?)",
        )
        .bind(user_id)
        .bind(it.position_id)
        .bind(if it.is_primary { 1 } else { 0 })
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    list_user_positions(pool, user_id).await
}
```

- [ ] **Step 2: 编译验证**

```powershell
cd services/user-service; cargo check
```
预期：编译通过。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/service/user_position_svc.rs
git commit -m "feat(user-service): add user_position_svc with primary uniqueness"
```

---

## Task 8: DTO 与 UserDTO 扩展

**Files:**
- Modify: `services/user-service/src/dto.rs`
- Modify: `services/user-service/src/models/user.rs`

- [ ] **Step 1: 新增请求 DTO**

在 `services/user-service/src/dto.rs` 末尾追加：
```rust
#[derive(Debug, Deserialize)]
pub struct CreateOrgUnitReq {
    pub code: String,
    pub name: String,
    pub parent_id: Option<u64>,
    #[serde(default)]
    pub sort: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrgUnitReq {
    pub name: String,
    pub parent_id: Option<u64>,
    pub sort: i32,
    pub status: i8,
}

#[derive(Debug, Deserialize)]
pub struct CreatePositionReq {
    pub code: String,
    pub name: String,
    pub org_unit_id: u64,
    #[serde(default)]
    pub level: i8,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePositionReq {
    pub name: String,
    pub org_unit_id: u64,
    pub level: i8,
    pub status: i8,
}

#[derive(Debug, Deserialize)]
pub struct AssignPositionRolesReq {
    pub role_ids: Vec<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AssignUserPositionsReq {
    pub items: Vec<UserPositionItem>,
}

#[derive(Debug, Deserialize)]
pub struct UserPositionItem {
    pub position_id: u64,
    #[serde(default)]
    pub is_primary: bool,
}
```

- [ ] **Step 2: UserDTO 加 positions 字段**

`services/user-service/src/models/user.rs`：
1. `use crate::models::user_position::UserPositionDTO;`
2. `UserDTO` 加 `pub positions: Vec<UserPositionDTO>,`
3. `into_dto` 增加 `positions: Vec::new()`（在后续 task 由 user_svc 填充）

- [ ] **Step 3: 编译验证**

```powershell
cd services/user-service; cargo check
```
预期：通过。

- [ ] **Step 4: 提交**

```bash
git add services/user-service/src/dto.rs services/user-service/src/models/user.rs
git commit -m "feat(user-service): add DTOs and UserDTO.positions"
```

---

## Task 9: 改造 user_svc 合并职位角色

**Files:**
- Modify: `services/user-service/src/service/user_svc.rs`

- [ ] **Step 1: 实现 fetch_position_role_codes 调用**

新增 `use crate::models::user_position::UserPositionDTO;` 与 `use crate::service::position_svc;` 与 `use crate::service::user_position_svc;`。

修改 `fetch_user_with_roles`：

```rust
pub async fn fetch_user_with_roles(pool: &DbPool, user_id: u64) -> Result<UserDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, email, phone, nickname, status, user_code, \
                profile_json, created_at, updated_at \
         FROM users WHERE id = ? AND status = 1",
    )
    .bind(user_id)
    .fetch_optional(p)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user id={user_id}")))?;

    let direct = fetch_user_role_codes(pool, user_id).await?;
    let pos_codes = fetch_user_position_role_codes(pool, user_id).await?;
    let mut seen = std::collections::HashSet::new();
    let roles: Vec<String> = direct
        .into_iter()
        .chain(pos_codes)
        .filter(|c| seen.insert(c.clone()))
        .collect();

    let positions = user_position_svc::list_user_positions(pool, user_id).await?;
    let mut dto = build_user_dto(row, roles);
    dto.positions = positions;
    Ok(dto)
}

async fn fetch_user_position_role_codes(
    pool: &DbPool,
    user_id: u64,
) -> Result<Vec<String>, AppError> {
    let DbPool::Mysql(p) = pool;
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT r.code FROM roles r \
         INNER JOIN position_roles pr ON pr.role_id = r.id \
         INNER JOIN user_positions up ON up.position_id = pr.position_id \
         WHERE up.user_id = ?",
    )
    .bind(user_id)
    .fetch_all(p)
    .await?;
    Ok(rows.into_iter().map(|(c,)| c).collect())
}
```

`fetch_user_by_code` 同步改造：先拿 row.id，再调用上面的逻辑。

`list_users` 循环中改 `fetch_user_with_roles` 即可。

- [ ] **Step 2: 编译验证**

```powershell
cd services/user-service; cargo check
```
预期：通过。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/service/user_svc.rs
git commit -m "feat(user-service): merge position-derived roles in fetch_user_with_roles"
```

---

## Task 10: 组织路由

**Files:**
- Create: `services/user-service/src/routes/org_units.rs`
- Modify: `services/user-service/src/routes/mod.rs`

- [ ] **Step 1: 实现组织路由**

`services/user-service/src/routes/org_units.rs`：
```rust
use crate::db::DbPool;
use crate::dto::{CreateOrgUnitReq, UpdateOrgUnitReq};
use crate::error::AppError;
use crate::service::org_svc;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct OrgUnitListQuery {
    pub parent_id: Option<u64>,
}

pub async fn list(
    State(pool): State<DbPool>,
    Query(q): Query<OrgUnitListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let items = org_svc::list_org_units(&pool, q.parent_id).await?;
    Ok(Json(items))
}

pub async fn create(
    State(pool): State<DbPool>,
    Json(req): Json<CreateOrgUnitReq>,
) -> Result<impl IntoResponse, AppError> {
    let dto = org_svc::create_org_unit(&pool, &req.code, &req.name, req.parent_id, req.sort).await?;
    Ok(Json(dto))
}

pub async fn get(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let dto = org_svc::get_org_unit(&pool, id).await?;
    Ok(Json(dto))
}

pub async fn update(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
    Json(req): Json<UpdateOrgUnitReq>,
) -> Result<impl IntoResponse, AppError> {
    let dto = org_svc::update_org_unit(&pool, id, &req.name, req.parent_id, req.sort, req.status)
        .await?;
    Ok(Json(dto))
}

pub async fn delete(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    org_svc::delete_org_unit(&pool, id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn tree(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let items = org_svc::get_org_tree(&pool, id).await?;
    Ok(Json(items))
}

pub fn router() -> axum::Router<DbPool> {
    axum::Router::new()
        .route("/org-units", axum::routing::get(list).post(create))
        .route("/org-units/:id", axum::routing::get(get).put(update).delete(delete))
        .route("/org-units/:id/tree", axum::routing::get(tree))
}
```

- [ ] **Step 2: 注册子模块**

`services/user-service/src/routes/mod.rs`：
```rust
pub mod auth;
pub mod health;
pub mod internal;
pub mod org_units;
pub mod permissions;
pub mod positions;
pub mod roles;
pub mod user_positions;
pub mod user_roles;
pub mod users;
```

- [ ] **Step 3: 编译验证**

`cargo check` 仅 `positions` / `user_positions` 路由未找到 error。

- [ ] **Step 4: 提交**

```bash
git add services/user-service/src/routes/org_units.rs services/user-service/src/routes/mod.rs
git commit -m "feat(user-service): add org_units routes"
```

---

## Task 11: 职位路由

**Files:**
- Create: `services/user-service/src/routes/positions.rs`

- [ ] **Step 1: 实现职位路由**

`services/user-service/src/routes/positions.rs`：
```rust
use crate::db::DbPool;
use crate::dto::{
    AssignPositionRolesReq, CreatePositionReq, UpdatePositionReq,
};
use crate::error::AppError;
use crate::service::position_svc;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PositionListQuery {
    pub org_unit_id: Option<u64>,
}

pub async fn list(
    State(pool): State<DbPool>,
    Query(q): Query<PositionListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let items = position_svc::list_positions(&pool, q.org_unit_id).await?;
    Ok(Json(items))
}

pub async fn create(
    State(pool): State<DbPool>,
    Json(req): Json<CreatePositionReq>,
) -> Result<impl IntoResponse, AppError> {
    let dto = position_svc::create_position(&pool, &req.code, &req.name, req.org_unit_id, req.level)
        .await?;
    Ok(Json(dto))
}

pub async fn get(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let dto = position_svc::get_position(&pool, id).await?;
    Ok(Json(dto))
}

pub async fn update(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
    Json(req): Json<UpdatePositionReq>,
) -> Result<impl IntoResponse, AppError> {
    let dto = position_svc::update_position(
        &pool, id, &req.name, req.org_unit_id, req.level, req.status,
    )
    .await?;
    Ok(Json(dto))
}

pub async fn delete(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    position_svc::delete_position(&pool, id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn assign_roles(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
    Json(req): Json<AssignPositionRolesReq>,
) -> Result<impl IntoResponse, AppError> {
    position_svc::assign_position_roles(&pool, id, &req.role_ids).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub fn router() -> axum::Router<DbPool> {
    axum::Router::new()
        .route("/positions", axum::routing::get(list).post(create))
        .route("/positions/:id", axum::routing::get(get).put(update).delete(delete))
        .route("/positions/:id/roles", axum::routing::put(assign_roles))
}
```

- [ ] **Step 2: 编译验证**

`cargo check` 仅 `user_positions` 路由未找到 error。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/routes/positions.rs
git commit -m "feat(user-service): add positions routes"
```

---

## Task 12: 用户-职位路由

**Files:**
- Create: `services/user-service/src/routes/user_positions.rs`

- [ ] **Step 1: 实现路由**

`services/user-service/src/routes/user_positions.rs`：
```rust
use crate::db::DbPool;
use crate::dto::AssignUserPositionsReq;
use crate::error::AppError;
use crate::service::user_position_svc::{self, PositionAssignItem};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};

pub async fn list(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let items = user_position_svc::list_user_positions(&pool, id).await?;
    Ok(Json(items))
}

pub async fn replace(
    State(pool): State<DbPool>,
    Path(id): Path<u64>,
    Json(req): Json<AssignUserPositionsReq>,
) -> Result<impl IntoResponse, AppError> {
    let items: Vec<PositionAssignItem> = req
        .items
        .into_iter()
        .map(|i| PositionAssignItem {
            position_id: i.position_id,
            is_primary: i.is_primary,
        })
        .collect();
    let items = user_position_svc::replace_user_positions(&pool, id, &items).await?;
    Ok(Json(items))
}

pub fn router() -> axum::Router<DbPool> {
    axum::Router::new()
        .route("/users/:id/positions", axum::routing::get(list).put(replace))
}
```

- [ ] **Step 2: 编译验证**

`cargo check` 通过。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/routes/user_positions.rs
git commit -m "feat(user-service): add user_positions routes"
```

---

## Task 13: 挂载新路由

**Files:**
- Modify: `services/user-service/src/routes/auth.rs`
- Modify: `services/user-service/src/main.rs`

- [ ] **Step 1: auth.rs 暴露总 router**

修改 `services/user-service/src/routes/auth.rs` 末尾：
```rust
pub fn router(state: AuthState) -> Router {
    Router::new()
        .route("/api/v1/auth/login", routing::post(login))
        .route("/api/v1/auth/register", routing::post(register))
        .route("/api/v1/auth/setup-status", routing::get(setup_status))
        .nest("/api/v1", crate::routes::users::router().merge(crate::routes::user_roles::router()))
        .merge(crate::routes::org_units::router())
        .merge(crate::routes::positions::router())
        .merge(crate::routes::user_positions::router())
        .merge(crate::routes::roles::router())
        .merge(crate::routes::permissions::router())
        .with_state(state)
}
```

- [ ] **Step 2: 编译验证**

```powershell
cd services/user-service; cargo check
```
预期：编译通过。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/src/routes/auth.rs
git commit -m "feat(user-service): mount org/position/user_position routers"
```

---

## Task 14: 测试 - 组织 CRUD + 树

**Files:**
- Create: `services/user-service/tests/org_position_test.rs`

- [ ] **Step 1: 写组织测试**

```rust
use sqlx::MySqlPool;

async fn pool() -> MySqlPool {
    let url = std::env::var("TEST_MYSQL_URL")
        .unwrap_or_else(|_| "mysql://root:root@localhost:3306/user_service_test".into());
    MySqlPool::connect(&url).await.expect("connect")
}

#[tokio::test]
async fn org_crud_and_tree() {
    let p = pool().await;
    sqlx::query("DELETE FROM org_units").execute(&p).await.unwrap();

    let root = user_service::service::org_svc::create_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        "ROOT", "Root", None, 0,
    ).await.unwrap();
    let child = user_service::service::org_svc::create_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        "CHILD", "Child", Some(root.id), 1,
    ).await.unwrap();
    let grand = user_service::service::org_svc::create_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        "GRAND", "Grand", Some(child.id), 0,
    ).await.unwrap();

    let tree = user_service::service::org_svc::get_org_tree(
        &user_service::db::DbPool::Mysql(p.clone()), root.id,
    ).await.unwrap();
    assert_eq!(tree.len(), 3);
    assert!(tree.iter().any(|o| o.code == "GRAND"));

    // 删除有子节点应失败
    let err = user_service::service::org_svc::delete_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()), root.id,
    ).await;
    assert!(err.is_err());

    user_service::service::org_svc::delete_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()), grand.id,
    ).await.unwrap();
    user_service::service::org_svc::delete_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()), child.id,
    ).await.unwrap();
    user_service::service::org_svc::delete_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()), root.id,
    ).await.unwrap();
}
```

- [ ] **Step 2: 跑测试**

```powershell
cd services/user-service; cargo test --test org_position_test org_crud_and_tree
```
预期：PASS。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/tests/org_position_test.rs
git commit -m "test(user-service): org unit CRUD + recursive tree"
```

---

## Task 15: 测试 - 职位 CRUD + 绑角色

**Files:**
- Modify: `services/user-service/tests/org_position_test.rs`

- [ ] **Step 1: 追加测试**

```rust
#[tokio::test]
async fn position_role_binding() {
    let p = pool().await;
    sqlx::query("DELETE FROM position_roles").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM positions").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM org_units").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM roles").execute(&p).await.unwrap();

    let ou = user_service::service::org_svc::create_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()), "OU", "OU", None, 0,
    ).await.unwrap();

    let role_admin: u64 = sqlx::query("INSERT INTO roles (code, name) VALUES ('admin', 'Admin')")
        .execute(&p).await.unwrap().last_insert_id();
    let role_user: u64 = sqlx::query("INSERT INTO roles (code, name) VALUES ('user', 'User')")
        .execute(&p).await.unwrap().last_insert_id();

    let pos = user_service::service::position_svc::create_position(
        &user_service::db::DbPool::Mysql(p.clone()),
        "PM", "ProjectManager", ou.id, 5,
    ).await.unwrap();
    user_service::service::position_svc::assign_position_roles(
        &user_service::db::DbPool::Mysql(p.clone()), pos.id, &[role_admin, role_user],
    ).await.unwrap();

    let fetched = user_service::service::position_svc::get_position(
        &user_service::db::DbPool::Mysql(p.clone()), pos.id,
    ).await.unwrap();
    assert_eq!(fetched.role_codes.len(), 2);
    assert!(fetched.role_codes.contains(&"admin".to_string()));
}
```

- [ ] **Step 2: 跑测试**

```powershell
cd services/user-service; cargo test --test org_position_test position_role_binding
```
预期：PASS。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/tests/org_position_test.rs
git commit -m "test(user-service): position CRUD + role binding"
```

---

## Task 16: 测试 - 用户职位主职唯一

**Files:**
- Modify: `services/user-service/tests/org_position_test.rs`

- [ ] **Step 1: 追加测试**

```rust
#[tokio::test]
async fn user_position_primary_uniqueness() {
    let p = pool().await;
    sqlx::query("DELETE FROM user_positions").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM positions").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM org_units").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM users WHERE id = 9999").execute(&p).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, user_code, status, password_hash) \
                 VALUES (9999, 'u_pos_test', 'UC9999', 1, 'x')")
        .execute(&p).await.unwrap();

    let ou = user_service::service::org_svc::create_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()), "OUP", "OUP", None, 0,
    ).await.unwrap();
    let p1 = user_service::service::position_svc::create_position(
        &user_service::db::DbPool::Mysql(p.clone()), "P1", "P1", ou.id, 0,
    ).await.unwrap();
    let p2 = user_service::service::position_svc::create_position(
        &user_service::db::DbPool::Mysql(p.clone()), "P2", "P2", ou.id, 0,
    ).await.unwrap();

    // 两个主职应被拒绝
    let err = user_service::service::user_position_svc::replace_user_positions(
        &user_service::db::DbPool::Mysql(p.clone()),
        9999,
        &[
            user_service::service::user_position_svc::PositionAssignItem {
                position_id: p1.id, is_primary: true,
            },
            user_service::service::user_position_svc::PositionAssignItem {
                position_id: p2.id, is_primary: true,
            },
        ],
    ).await;
    assert!(err.is_err());

    // 一个主职 + 一个普通成功
    let items = user_service::service::user_position_svc::replace_user_positions(
        &user_service::db::DbPool::Mysql(p.clone()),
        9999,
        &[
            user_service::service::user_position_svc::PositionAssignItem {
                position_id: p1.id, is_primary: true,
            },
            user_service::service::user_position_svc::PositionAssignItem {
                position_id: p2.id, is_primary: false,
            },
        ],
    ).await.unwrap();
    assert_eq!(items.len(), 2);
    let primary_count = items.iter().filter(|i| i.is_primary).count();
    assert_eq!(primary_count, 1);
}
```

- [ ] **Step 2: 跑测试**

```powershell
cd services/user-service; cargo test --test org_position_test user_position_primary_uniqueness
```
预期：PASS。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/tests/org_position_test.rs
git commit -m "test(user-service): user position primary uniqueness"
```

---

## Task 17: 测试 - 授权合并

**Files:**
- Create: `services/user-service/tests/authz_combination_test.rs`

- [ ] **Step 1: 写授权合并测试**

```rust
#[tokio::test]
async fn user_gets_union_of_direct_and_position_roles() {
    let p = pool().await;
    // 清场
    sqlx::query("DELETE FROM role_permissions").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM permissions").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM position_roles").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM user_positions").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM user_roles").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM positions").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM org_units").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM roles").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM users WHERE id = 8888").execute(&p).await.unwrap();

    sqlx::query("INSERT INTO users (id, username, user_code, status, password_hash) \
                 VALUES (8888, 'u_authz', 'UC8888', 1, 'x')")
        .execute(&p).await.unwrap();

    let role_a: u64 = sqlx::query("INSERT INTO roles (code, name) VALUES ('role_a', 'RoleA')")
        .execute(&p).await.unwrap().last_insert_id();
    let role_b: u64 = sqlx::query("INSERT INTO roles (code, name) VALUES ('role_b', 'RoleB')")
        .execute(&p).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO permissions (code, name, resource, action) VALUES ('perm_a', 'PA', 'r', 'read')")
        .execute(&p).await.unwrap();
    sqlx::query("INSERT INTO permissions (code, name, resource, action) VALUES ('perm_b', 'PB', 'r', 'read')")
        .execute(&p).await.unwrap();
    sqlx::query("INSERT INTO role_permissions (role_id, permission_id) SELECT ?, id FROM permissions WHERE code='perm_a'").bind(role_a).execute(&p).await.unwrap();
    sqlx::query("INSERT INTO role_permissions (role_id, permission_id) SELECT ?, id FROM permissions WHERE code='perm_b'").bind(role_b).execute(&p).await.unwrap();

    // 直接授予 role_a
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (8888, ?)")
        .bind(role_a).execute(&p).await.unwrap();

    // 职位绑 role_b
    let ou = user_service::service::org_svc::create_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()), "OA", "OA", None, 0,
    ).await.unwrap();
    let pos = user_service::service::position_svc::create_position(
        &user_service::db::DbPool::Mysql(p.clone()), "PA", "PA", ou.id, 0,
    ).await.unwrap();
    user_service::service::position_svc::assign_position_roles(
        &user_service::db::DbPool::Mysql(p.clone()), pos.id, &[role_b],
    ).await.unwrap();
    user_service::service::user_position_svc::replace_user_positions(
        &user_service::db::DbPool::Mysql(p.clone()), 8888,
        &[user_service::service::user_position_svc::PositionAssignItem {
            position_id: pos.id, is_primary: true,
        }],
    ).await.unwrap();

    let dto = user_service::service::user_svc::fetch_user_with_roles(
        &user_service::db::DbPool::Mysql(p.clone()), 8888,
    ).await.unwrap();
    assert!(dto.roles.contains(&"role_a".to_string()));
    assert!(dto.roles.contains(&"role_b".to_string()));
    assert_eq!(dto.positions.len(), 1);
}
```

- [ ] **Step 2: 跑测试**

```powershell
cd services/user-service; cargo test --test authz_combination_test
```
预期：PASS。

- [ ] **Step 3: 提交**

```bash
git add services/user-service/tests/authz_combination_test.rs
git commit -m "test(user-service): authorization combines direct + position roles"
```

---

## Task 18: 全量验证

- [ ] **Step 1: 格式化**

```powershell
cd services/user-service; cargo fmt --check
```

- [ ] **Step 2: clippy**

```powershell
cd services/user-service; cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 3: 全量 test**

```powershell
cd services/user-service; cargo test
```
预期：全 PASS。

- [ ] **Step 4: 最终提交（如有调整）**

```bash
git add -u
git commit -m "chore(user-service): fmt + clippy cleanup"
```

---

## Self-Review

**Spec coverage:**
- 四张表 → Task 1
- 树型组织 → Task 5 (recursive CTE)
- 职位绑角色 → Task 6
- 用户多职位主职唯一 → Task 7
- 迁移脚本 → Task 1
- 路由三类 → Task 10/11/12/13
- DTO → Task 8
- UserDTO.positions + 授权合并 → Task 9
- 测试 → Task 14-17

**Placeholder scan:** 无 TBD/TODO。
**Type consistency:** `PositionAssignItem` 在 user_position_svc 与 routes 间一致；`UserPositionDTO.is_primary` 一致使用 bool。

---

## 执行方式

计划已写入 `docs/superpowers/plans/2026-06-11-user-service-org-position-plan.md`。

**两种执行方式可选：**

1. **Subagent-Driven（推荐）** — 每个 Task 派新 subagent，逐 Task 评审。
2. **Inline Execution** — 在当前会话按 Task 顺序执行，含评审 checkpoint。

请选择执行方式。
