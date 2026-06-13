use sqlx::MySqlPool;

async fn pool() -> MySqlPool {
    let url = std::env::var("TEST_MYSQL_URL")
        .unwrap_or_else(|_| "mysql://root:root@localhost:3306/user_service_test".into());
    MySqlPool::connect(&url).await.expect("connect")
}

#[tokio::test]
async fn user_gets_union_of_direct_and_position_roles() {
    let p = pool().await;
    sqlx::query("DELETE FROM role_permissions")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DELETE FROM permissions")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DELETE FROM position_roles")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DELETE FROM user_positions")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DELETE FROM user_roles")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DELETE FROM positions")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DELETE FROM org_units")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DELETE FROM roles").execute(&p).await.unwrap();
    sqlx::query("DELETE FROM users WHERE id = 8888")
        .execute(&p)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO users (id, username, user_code, status, password_hash) \
         VALUES (8888, 'u_authz', 'UC8888', 1, 'x')",
    )
    .execute(&p)
    .await
    .unwrap();

    let role_a: u64 = sqlx::query("INSERT INTO roles (code, name) VALUES ('role_a', 'RoleA')")
        .execute(&p)
        .await
        .unwrap()
        .last_insert_id();
    let role_b: u64 = sqlx::query("INSERT INTO roles (code, name) VALUES ('role_b', 'RoleB')")
        .execute(&p)
        .await
        .unwrap()
        .last_insert_id();
    sqlx::query(
        "INSERT INTO permissions (code, name, resource, action) VALUES ('perm_a', 'PA', 'r', 'read')",
    )
    .execute(&p)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO permissions (code, name, resource, action) VALUES ('perm_b', 'PB', 'r', 'read')",
    )
    .execute(&p)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO role_permissions (role_id, permission_id) \
         SELECT ?, id FROM permissions WHERE code='perm_a'",
    )
    .bind(role_a)
    .execute(&p)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO role_permissions (role_id, permission_id) \
         SELECT ?, id FROM permissions WHERE code='perm_b'",
    )
    .bind(role_b)
    .execute(&p)
    .await
    .unwrap();

    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (8888, ?)")
        .bind(role_a)
        .execute(&p)
        .await
        .unwrap();

    let ou = user_service::service::org_svc::create_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        "OA",
        "OA",
        None,
        0,
    )
    .await
    .unwrap();
    let pos = user_service::service::position_svc::create_position(
        &user_service::db::DbPool::Mysql(p.clone()),
        "PA",
        "PA",
        ou.id,
        0,
    )
    .await
    .unwrap();
    user_service::service::position_svc::assign_position_roles(
        &user_service::db::DbPool::Mysql(p.clone()),
        pos.id,
        &[role_b],
    )
    .await
    .unwrap();
    user_service::service::user_position_svc::replace_user_positions(
        &user_service::db::DbPool::Mysql(p.clone()),
        8888,
        &[
            user_service::service::user_position_svc::PositionAssignItem {
                position_id: pos.id,
                is_primary: true,
            },
        ],
    )
    .await
    .unwrap();

    let dto = user_service::service::user_svc::fetch_user_with_roles(
        &user_service::db::DbPool::Mysql(p.clone()),
        8888,
    )
    .await
    .unwrap();
    assert!(dto.roles.contains(&"role_a".to_string()));
    assert!(dto.roles.contains(&"role_b".to_string()));
    assert_eq!(dto.positions.len(), 1);
}
