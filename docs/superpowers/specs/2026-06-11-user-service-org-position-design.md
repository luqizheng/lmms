# User Service 组织单元与职位设计

## 概述

为 `user-service` 增加组织单元（OrgUnit）与职位（Position）两类实体，支撑：
- 用户数据归属（部门/职位记录）
- 通过职位自动获得角色授权（职位是角色容器）

## 范围

- 新增 `org_units` / `positions` / `position_roles` / `user_positions` 四张表
- 新增组织、职位、用户-职位绑定相关 routes
- 调整 `UserDTO` 与授权查询（直接角色 ∪ 用户职位→职位角色）
- 一次性迁移脚本处理 `user.profile_json.dept/title` 历史数据

不在范围：审批流、汇报链继承、跨部门数据权限。

## 数据模型

### org_units（组织单元，树型）

```sql
CREATE TABLE org_units (
  id          BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  code        VARCHAR(64) NOT NULL UNIQUE,
  name        VARCHAR(128) NOT NULL,
  parent_id   BIGINT UNSIGNED NULL,
  sort        INT NOT NULL DEFAULT 0,
  status      TINYINT NOT NULL DEFAULT 1,
  created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  KEY idx_parent (parent_id),
  CONSTRAINT fk_org_parent FOREIGN KEY (parent_id) REFERENCES org_units(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

### positions（职位）

```sql
CREATE TABLE positions (
  id          BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  code        VARCHAR(64) NOT NULL UNIQUE,
  name        VARCHAR(128) NOT NULL,
  org_unit_id BIGINT UNSIGNED NOT NULL,
  level       TINYINT NOT NULL DEFAULT 0,
  status      TINYINT NOT NULL DEFAULT 1,
  created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  KEY idx_org (org_unit_id),
  CONSTRAINT fk_pos_org FOREIGN KEY (org_unit_id) REFERENCES org_units(id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

### position_roles（职位 → 角色，多对多）

```sql
CREATE TABLE position_roles (
  position_id BIGINT UNSIGNED NOT NULL,
  role_id     BIGINT UNSIGNED NOT NULL,
  PRIMARY KEY (position_id, role_id),
  KEY idx_role (role_id),
  CONSTRAINT fk_pr_pos FOREIGN KEY (position_id) REFERENCES positions(id) ON DELETE CASCADE,
  CONSTRAINT fk_pr_role FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

### user_positions（用户 → 职位，多对多，含主职）

```sql
CREATE TABLE user_positions (
  user_id     BIGINT UNSIGNED NOT NULL,
  position_id BIGINT UNSIGNED NOT NULL,
  is_primary  TINYINT NOT NULL DEFAULT 0,
  assigned_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (user_id, position_id),
  KEY idx_pos (position_id),
  KEY idx_primary_user (user_id, is_primary),
  CONSTRAINT fk_up_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_up_pos  FOREIGN KEY (position_id) REFERENCES positions(id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

## 关键设计决策

### 授权链

`User → user_positions → positions → position_roles → roles → role_permissions → permissions`

用户最终权限码 = 直接绑定的角色权限码 ∪ 所有职位带来的角色权限码（去重）。

### 主职唯一性

写 `user_positions` 时若 `is_primary=1`：
1. 事务开始
2. `UPDATE user_positions SET is_primary=0 WHERE user_id=? AND is_primary=1`
3. `INSERT user_positions (user_id, position_id, is_primary=1)`
4. 提交

### 组织树查询

使用 MySQL 8.0 `WITH RECURSIVE` CTE 查询子树或祖先链。

### 废弃旧字段

`user.profile_json.dept` / `.title` 不再写入；迁移脚本一次性把历史值建成 `org_units` + `positions` + `user_positions`，字段保留但不再读取（兼容）。

## 路由

### 组织单元

| Method | Path | 说明 |
|---|---|---|
| GET | `/org-units` | 列表（可按 parent_id 过滤） |
| POST | `/org-units` | 创建（parent_id 可空） |
| GET | `/org-units/{id}` | 详情 |
| PUT | `/org-units/{id}` | 更新 |
| DELETE | `/org-units/{id}` | 删除（有子节点/职位时拒绝） |
| GET | `/org-units/{id}/tree` | 递归子树 |

### 职位

| Method | Path | 说明 |
|---|---|---|
| GET | `/positions` | 列表（可按 org_unit_id 过滤） |
| POST | `/positions` | 创建 |
| GET | `/positions/{id}` | 详情（含绑定的角色 code） |
| PUT | `/positions/{id}` | 更新 |
| DELETE | `/positions/{id}` | 删除（被 user_positions 引用时拒绝） |
| PUT | `/positions/{id}/roles` | 绑定/解绑角色（body: `{role_ids: [...]}` 全量替换） |

### 用户 → 职位

| Method | Path | 说明 |
|---|---|---|
| GET | `/users/{id}/positions` | 列出用户职位（含主职标记） |
| PUT | `/users/{id}/positions` | 全量重置（body: `{items:[{position_id, is_primary}, ...]}` 事务保证唯一主职） |

## DTO

```rust
#[derive(Serialize)]
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

#[derive(Serialize)]
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

#[derive(Serialize)]
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

`UserDTO` 增加 `positions: Vec<UserPositionDTO>`。

## 授权查询改造

`user_svc::fetch_user_with_roles` 改造为：

```rust
pub async fn fetch_user_with_roles(pool: &DbPool, user_id: u64) -> Result<UserDTO> {
    let direct_roles = /* 现有 user_roles JOIN roles */;
    let position_roles = /* user_positions JOIN position_roles JOIN roles
                            WHERE user_id = ? */;
    let all_roles: Vec<String> = direct_roles.into_iter()
        .chain(position_roles)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    // ...
}
```

`auth_svc` 中权限码查询同步合并两条链路。

## 迁移脚本

`scripts/migrations/2026_06_11_org_position_init.sql`：

1. 创建四张新表
2. 扫描 `users.profile_json`：
   - `dept` 不为空 → UPSERT `org_units(code=name)`，记 id
   - `title` 不为空 → UPSERT `positions(code=name, org_unit_id=前一步 dept)`
   - 写 `user_positions(user_id, position_id, is_primary=1)`
3. 不删除 `profile_json.dept/title` 列（兼容）

迁移幂等：重复执行无副作用。

## 测试

`tests/org_position_test.rs`：
- 组织 CRUD + 树查询
- 职位 CRUD + 绑角色
- 用户职位绑定与主职唯一性
- 授权查询：仅职位角色 vs 直接角色 vs 并集

`tests/authz_combination_test.rs`：
- 直接授予角色 A + 职位授予角色 B → 用户同时拥有 A、B 权限
- 同名角色去重

## 风险

- 迁移脚本对历史脏数据宽容度有限，需要日志报告无法映射的记录
- 主职唯一性靠事务保证，应用层与 DB 层双重约束
- `positions` 被 `user_positions` 引用时删除需明确拒绝，避免级联清空授权
