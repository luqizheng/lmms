use common_config::{HotConfig, Secret, SensitiveFields};
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn hot_config_get_returns_initial_value() {
    let config = HotConfig::new(42u32).await;
    assert_eq!(config.get().await, 42);
}

#[tokio::test]
async fn hot_config_update_changes_value() {
    let config = HotConfig::new(10u32).await;
    
    config.update(20).await;
    assert_eq!(config.get().await, 20);
}

#[tokio::test]
async fn hot_config_callback_invoked_on_update() {
    let config = HotConfig::new(100u32).await;
    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = call_count.clone();
    
    config.on_change(move |val| {
        call_count_clone.fetch_add(1, Ordering::SeqCst);
        assert_eq!(*val, 200);
    }).await;
    
    config.update(200).await;
    
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn hot_config_multiple_callbacks() {
    let config = HotConfig::new("initial".to_string()).await;
    let count1 = Arc::new(AtomicUsize::new(0));
    let count2 = Arc::new(AtomicUsize::new(0));
    let c1 = count1.clone();
    let c2 = count2.clone();
    
    config.on_change(move |_| { c1.fetch_add(1, Ordering::SeqCst); }).await;
    config.on_change(move |_| { c2.fetch_add(1, Ordering::SeqCst); }).await;
    
    config.update("updated".to_string()).await;
    
    assert_eq!(count1.load(Ordering::SeqCst), 1);
    assert_eq!(count2.load(Ordering::SeqCst), 1);
}

use std::sync::Arc;

#[test]
fn secret_display_is_masked() {
    let secret = Secret("my_password".to_string());
    assert_eq!(format!("{}", secret), "******");
}

#[test]
fn secret_debug_is_masked() {
    let secret = Secret("my_password".to_string());
    assert_eq!(format!("{:?}", secret), "Secret(******)");
}

#[test]
fn secret_inner_returns_original() {
    let secret = Secret("my_password".to_string());
    assert_eq!(secret.inner(), "my_password");
}

#[test]
fn sensitive_fields_default_empty() {
    let fields = SensitiveFields::new();
    assert!(!fields.is_sensitive("password"));
    assert!(!fields.is_sensitive("token"));
}

#[test]
fn sensitive_fields_add_marks_field() {
    let mut fields = SensitiveFields::new();
    fields.add("password");
    fields.add("secret_key");
    
    assert!(fields.is_sensitive("password"));
    assert!(fields.is_sensitive("secret_key"));
    assert!(!fields.is_sensitive("username"));
}

#[test]
fn sensitive_fields_mask_value() {
    let fields = SensitiveFields::new();
    fields.add("password");
    
    assert_eq!(fields.mask_value("password", "secret123"), "******");
    assert_eq!(fields.mask_value("username", "john"), "john");
}
