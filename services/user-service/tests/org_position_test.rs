use sqlx::MySqlPool;

async fn pool() -> MySqlPool {
    let url = std::env::var("TEST_MYSQL_URL")
        .unwrap_or_else(|_| "mysql://root:root@localhost:3306/user_service_test".into());
    MySqlPool::connect(&url).await.expect("connect")
}

#[tokio::test]
async fn org_crud_and_tree() {
    let p = pool().await;
    sqlx::query("DELETE FROM org_units")
        .execute(&p)
        .await
        .unwrap();

    let root = user_service::service::org_svc::create_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        "ROOT",
        "Root",
        None,
        0,
    )
    .await
    .unwrap();
    let child = user_service::service::org_svc::create_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        "CHILD",
        "Child",
        Some(root.id),
        1,
    )
    .await
    .unwrap();
    let grand = user_service::service::org_svc::create_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        "GRAND",
        "Grand",
        Some(child.id),
        0,
    )
    .await
    .unwrap();

    let tree = user_service::service::org_svc::get_org_tree(
        &user_service::db::DbPool::Mysql(p.clone()),
        root.id,
    )
    .await
    .unwrap();
    assert_eq!(tree.len(), 3);
    assert!(tree.iter().any(|o| o.code == "GRAND"));

    let err = user_service::service::org_svc::delete_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        root.id,
    )
    .await;
    assert!(err.is_err());

    user_service::service::org_svc::delete_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        grand.id,
    )
    .await
    .unwrap();
    user_service::service::org_svc::delete_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        child.id,
    )
    .await
    .unwrap();
    user_service::service::org_svc::delete_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        root.id,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn position_role_binding() {
    let p = pool().await;
    sqlx::query("DELETE FROM position_roles")
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

    let ou = user_service::service::org_svc::create_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        "OU",
        "OU",
        None,
        0,
    )
    .await
    .unwrap();

    let role_admin: u64 = sqlx::query("INSERT INTO roles (code, name) VALUES ('admin', 'Admin')")
        .execute(&p)
        .await
        .unwrap()
        .last_insert_id();
    let role_user: u64 = sqlx::query("INSERT INTO roles (code, name) VALUES ('user', 'User')")
        .execute(&p)
        .await
        .unwrap()
        .last_insert_id();

    let pos = user_service::service::position_svc::create_position(
        &user_service::db::DbPool::Mysql(p.clone()),
        "PM",
        "ProjectManager",
        ou.id,
        5,
    )
    .await
    .unwrap();
    user_service::service::position_svc::assign_position_roles(
        &user_service::db::DbPool::Mysql(p.clone()),
        pos.id,
        &[role_admin, role_user],
    )
    .await
    .unwrap();

    let fetched = user_service::service::position_svc::get_position(
        &user_service::db::DbPool::Mysql(p.clone()),
        pos.id,
    )
    .await
    .unwrap();
    assert_eq!(fetched.role_codes.len(), 2);
    assert!(fetched.role_codes.contains(&"admin".to_string()));
}

#[tokio::test]
async fn user_position_primary_uniqueness() {
    let p = pool().await;
    sqlx::query("DELETE FROM user_positions")
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
    sqlx::query("DELETE FROM users WHERE id = 9999")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, user_code, status, password_hash) \
         VALUES (9999, 'u_pos_test', 'UC9999', 1, 'x')",
    )
    .execute(&p)
    .await
    .unwrap();

    let ou = user_service::service::org_svc::create_org_unit(
        &user_service::db::DbPool::Mysql(p.clone()),
        "OUP",
        "OUP",
        None,
        0,
    )
    .await
    .unwrap();
    let p1 = user_service::service::position_svc::create_position(
        &user_service::db::DbPool::Mysql(p.clone()),
        "P1",
        "P1",
        ou.id,
        0,
    )
    .await
    .unwrap();
    let p2 = user_service::service::position_svc::create_position(
        &user_service::db::DbPool::Mysql(p.clone()),
        "P2",
        "P2",
        ou.id,
        0,
    )
    .await
    .unwrap();

    let err = user_service::service::user_position_svc::replace_user_positions(
        &user_service::db::DbPool::Mysql(p.clone()),
        9999,
        &[
            user_service::service::user_position_svc::PositionAssignItem {
                position_id: p1.id,
                is_primary: true,
            },
            user_service::service::user_position_svc::PositionAssignItem {
                position_id: p2.id,
                is_primary: true,
            },
        ],
    )
    .await;
    assert!(err.is_err());

    let items = user_service::service::user_position_svc::replace_user_positions(
        &user_service::db::DbPool::Mysql(p.clone()),
        9999,
        &[
            user_service::service::user_position_svc::PositionAssignItem {
                position_id: p1.id,
                is_primary: true,
            },
            user_service::service::user_position_svc::PositionAssignItem {
                position_id: p2.id,
                is_primary: false,
            },
        ],
    )
    .await
    .unwrap();
    assert_eq!(items.len(), 2);
    let primary_count = items.iter().filter(|i| i.is_primary).count();
    assert_eq!(primary_count, 1);
}
