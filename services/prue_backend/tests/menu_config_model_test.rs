use prue_backend::dto::{validate_routes_json, ApiResponse, CreateMenuConfigRequest};
use serde_json::json;

#[test]
fn api_response_uses_pure_admin_shape() {
    let response = ApiResponse::ok(json!([]));
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(value["code"], 0);
    assert_eq!(value["message"], "ok");
    assert!(value.get("data").is_some());
}

#[test]
fn create_menu_config_serializes_roles_and_auths_as_strings() {
    let req = CreateMenuConfigRequest {
        name: "main".to_string(),
        description: Some("default menu".to_string()),
        created_by: Some("admin".to_string()),
        routes_json: json!([
            {
                "path": "/system",
                "name": "System",
                "meta": {
                    "roles": ["admin"],
                    "auths": ["menu:list"]
                }
            }
        ]),
    };

    let value = serde_json::to_value(req).unwrap();
    assert_eq!(value["routes_json"][0]["meta"]["roles"][0], "admin");
    assert_eq!(value["routes_json"][0]["meta"]["auths"][0], "menu:list");
}

#[test]
fn validate_routes_json_accepts_nested_routes() {
    let routes = json!([
        {
            "path": "/system",
            "name": "System",
            "meta": { "roles": ["admin"] },
            "children": [
                {
                    "path": "/system/menu/index",
                    "name": "SystemMenu",
                    "component": "system/menu/index",
                    "meta": { "auths": ["menu:list", "menu:create"] }
                }
            ]
        }
    ]);

    assert!(validate_routes_json(&routes).is_ok());
}

#[test]
fn validate_routes_json_rejects_duplicate_path() {
    let routes = json!([
        { "path": "/system", "name": "System" },
        { "path": "/system", "name": "SystemCopy" }
    ]);

    let err = validate_routes_json(&routes).unwrap_err();
    assert!(err.contains("duplicate route.path"));
}

#[test]
fn validate_routes_json_rejects_non_string_roles() {
    let routes = json!([
        { "path": "/system", "name": "System", "meta": { "roles": [1] } }
    ]);

    let err = validate_routes_json(&routes).unwrap_err();
    assert!(err.contains("route.meta.roles"));
}
