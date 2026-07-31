use chrono::Utc;
use config_service::models::config_item::ConfigItem;
use digua_jwt_ctx::Principal;
use std::collections::HashSet;

fn make_principal(perms: &[&str]) -> Principal {
    Principal {
        jti: "test".into(),
        user_id: "u1".into(),
        user_code: "u1".into(),
        tenant_id: 1,
        cross_tenant: false,
        dept_id: None,
        roles: vec!["admin".into()],
        permissions: perms.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
    }
}

fn make_item(is_secret: bool, value: &str) -> ConfigItem {
    ConfigItem {
        id: "i1".into(),
        namespace_id: "ns1".into(),
        key: "k".into(),
        value: value.into(),
        value_type: "string".into(),
        is_secret,
        description: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        value_masked: false,
    }
}

/// 复制自 routes/config.rs 的 reveal policy 逻辑（保持同步）：
/// is_secret=true 且 caller 无 CFG_CONFIG_REVEAL → value="********" + value_masked=true
fn apply_reveal_policy(item: &mut ConfigItem, principal: &Principal) {
    const MASK: &str = "********";
    const REVEAL_PERM: &str = "CFG_CONFIG_REVEAL";
    if item.is_secret && !principal.has_permission(REVEAL_PERM) {
        item.value = MASK.to_string();
        item.value_masked = true;
    }
}

#[test]
fn non_secret_value_passes_through() {
    let principal = make_principal(&[]);
    let mut item = make_item(false, "plain-value");
    apply_reveal_policy(&mut item, &principal);
    assert_eq!(item.value, "plain-value");
    assert!(!item.value_masked);
}

#[test]
fn secret_without_reveal_perm_is_masked() {
    let principal = make_principal(&["CFG_CONFIG_READ"]);
    let mut item = make_item(true, "super-secret");
    apply_reveal_policy(&mut item, &principal);
    assert_eq!(item.value, "********");
    assert!(item.value_masked);
}

#[test]
fn secret_with_reveal_perm_returns_plaintext() {
    let principal = make_principal(&["CFG_CONFIG_REVEAL"]);
    let mut item = make_item(true, "super-secret");
    apply_reveal_policy(&mut item, &principal);
    assert_eq!(item.value, "super-secret");
    assert!(!item.value_masked);
}

#[test]
fn principal_has_permission_helper() {
    let p = make_principal(&["CFG_CONFIG_REVEAL", "CFG_NAMESPACE_READ"]);
    assert!(p.has_permission("CFG_CONFIG_REVEAL"));
    assert!(!p.has_permission("CFG_NAMESPACE_DELETE"));
}