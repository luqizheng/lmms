use config_service::models::{
    CreateConfigItem, CreateNamespace, QueryConfigItems, QueryNamespaces,
};

#[test]
fn create_namespace_serialization() {
    let ns = CreateNamespace {
        app: "myapp".to_string(),
        env: "prod".to_string(),
        profile: Some("default".to_string()),
        description: Some("Production config".to_string()),
    };

    let json = serde_json::to_string(&ns).unwrap();
    assert!(json.contains("myapp"));
    assert!(json.contains("prod"));
    assert!(json.contains("default"));
}

#[test]
fn create_namespace_minimal() {
    let ns = CreateNamespace {
        app: "simple".to_string(),
        env: "dev".to_string(),
        profile: None,
        description: None,
    };

    let json = serde_json::to_string(&ns).unwrap();
    assert!(json.contains("simple"));
    assert!(json.contains("dev"));
}

#[test]
fn query_namespaces_defaults() {
    let query = QueryNamespaces {
        app: None,
        env: None,
        page: None,
        size: None,
    };

    assert!(query.page.is_none());
    assert!(query.size.is_none());
}

#[test]
fn create_config_item_with_types() {
    let item = CreateConfigItem {
        namespace_id: "ns-123".to_string(),
        key: "database.host".to_string(),
        value: "localhost".to_string(),
        value_type: "string".to_string(),
        is_secret: false,
        description: Some("Database host".to_string()),
    };

    let json = serde_json::to_string(&item).unwrap();
    assert!(json.contains("database.host"));
    assert!(json.contains("localhost"));
}

#[test]
fn create_secret_config_item() {
    let item = CreateConfigItem {
        namespace_id: "ns-123".to_string(),
        key: "database.password".to_string(),
        value: "secret123".to_string(),
        value_type: "string".to_string(),
        is_secret: true,
        description: None,
    };

    assert!(item.is_secret);
    assert_eq!(item.key, "database.password");
}

#[test]
fn query_config_items_with_filter() {
    let query = QueryConfigItems {
        namespace_id: Some("ns-123".to_string()),
        key: Some("database.".to_string()),
        page: Some(1),
        size: Some(10),
    };

    assert_eq!(query.namespace_id, Some("ns-123".to_string()));
    assert_eq!(query.size, Some(10));
}
