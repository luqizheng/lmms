use digua_common_db::init_pool;
use prue_backend::{
    db,
    dto::{CreateMenuConfigRequest, UpdateMenuConfigRequest},
    error::AppError,
    service::MenuConfigService,
    state::AppState,
};
use serde_json::json;

async fn fresh_service() -> MenuConfigService {
    let pool = init_pool("sqlite::memory:").await.expect("init pool");
    db::ensure_schema(&pool).await.expect("ensure schema");
    let state = AppState::new(pool);
    MenuConfigService::new(state)
}

fn sample_routes() -> serde_json::Value {
    json!([
        {
            "path": "/system",
            "name": "System",
            "meta": {
                "title": "System",
                "_menu": { "id": 1, "menuType": 0 }
            },
            "children": [
                {
                    "path": "/system/menu",
                    "name": "SystemMenu",
                    "meta": {
                        "title": "Menu",
                        "_menu": { "id": 2, "menuType": 1 }
                    }
                }
            ]
        }
    ])
}

#[tokio::test]
async fn create_assigns_non_empty_etag() {
    let svc = fresh_service().await;
    let detail = svc
        .create(CreateMenuConfigRequest {
            name: "main".into(),
            description: None,
            routes_json: sample_routes(),
            created_by: Some("admin".into()),
        })
        .await
        .expect("create ok");
    assert_eq!(detail.routes_etag.len(), 64);
    assert!(!detail.routes_etag.contains(' '));
}

#[tokio::test]
async fn update_with_matching_etag_rotates_etag() {
    let svc = fresh_service().await;
    let detail = svc
        .create(CreateMenuConfigRequest {
            name: "main".into(),
            description: None,
            routes_json: sample_routes(),
            created_by: None,
        })
        .await
        .unwrap();
    let original_etag = detail.routes_etag.clone();

    let new_routes = json!([
        { "path": "/dashboard", "name": "Dashboard", "meta": { "title": "Home" } }
    ]);
    let updated = svc
        .update(
            &detail.id,
            UpdateMenuConfigRequest {
                name: None,
                description: None,
                routes_json: Some(new_routes),
                if_match_etag: Some(original_etag.clone()),
            },
        )
        .await
        .expect("update ok");

    assert_ne!(updated.routes_etag, original_etag);
    assert_eq!(updated.routes_etag.len(), 64);
}

#[tokio::test]
async fn update_with_stale_etag_returns_etag_conflict() {
    let svc = fresh_service().await;
    let detail = svc
        .create(CreateMenuConfigRequest {
            name: "main".into(),
            description: None,
            routes_json: sample_routes(),
            created_by: None,
        })
        .await
        .unwrap();

    let err = svc
        .update(
            &detail.id,
            UpdateMenuConfigRequest {
                name: None,
                description: None,
                routes_json: Some(json!([{ "path": "/x", "name": "X" }])),
                if_match_etag: Some("definitely-not-the-real-etag".into()),
            },
        )
        .await
        .expect_err("expected conflict");

    match err {
        AppError::EtagConflict {
            current_etag,
            updated_at: _,
        } => {
            assert_eq!(current_etag, detail.routes_etag);
        }
        other => panic!("expected EtagConflict, got {other:?}"),
    }
}

#[tokio::test]
async fn update_without_if_match_etag_returns_validation_error() {
    let svc = fresh_service().await;
    let detail = svc
        .create(CreateMenuConfigRequest {
            name: "main".into(),
            description: None,
            routes_json: sample_routes(),
            created_by: None,
        })
        .await
        .unwrap();

    let err = svc
        .update(
            &detail.id,
            UpdateMenuConfigRequest {
                name: Some("rename".into()),
                description: None,
                routes_json: None,
                if_match_etag: None,
            },
        )
        .await
        .expect_err("expected validation");

    match err {
        AppError::Validation(msg) => assert!(msg.contains("if_match_etag")),
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[tokio::test]
async fn active_routes_strips_menu_field_recursively() {
    let svc = fresh_service().await;
    let detail = svc
        .create(CreateMenuConfigRequest {
            name: "main".into(),
            description: None,
            routes_json: sample_routes(),
            created_by: None,
        })
        .await
        .unwrap();

    svc.publish(&detail.id).await.expect("publish");

    let active = svc.active_routes().await.expect("active");
    let serialized = serde_json::to_string(&active).unwrap();
    assert!(
        !serialized.contains("_menu"),
        "active_routes should not contain _menu: {}",
        serialized
    );

    // detail should still preserve _menu for the menu-management UI
    let still_has_menu = svc.get(&detail.id).await.unwrap();
    let detail_serialized = serde_json::to_string(&still_has_menu.routes_json).unwrap();
    assert!(detail_serialized.contains("_menu"));
}
