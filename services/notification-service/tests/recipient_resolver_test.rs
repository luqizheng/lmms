use notification_service::models::ScopeType;
use notification_service::recipient_resolver::expand_local;
use std::collections::HashMap;

#[test]
fn expand_dedupes_users_across_scopes() {
    let mut user_to_roles: HashMap<String, Vec<String>> = HashMap::new();
    user_to_roles.insert("10".into(), vec!["1".into()]);
    user_to_roles.insert("11".into(), vec!["2".into()]);
    let mut user_to_dept: HashMap<String, String> = HashMap::new();
    user_to_dept.insert("10".into(), "100".into());
    user_to_dept.insert("12".into(), "100".into());
    let scopes = vec![
        (ScopeType::User, "10".into()),
        (ScopeType::Role, "1".into()),
        (ScopeType::Dept, "100".into()),
        (ScopeType::User, "11".into()),
    ];
    let s = expand_local(&scopes, &user_to_roles, &user_to_dept);
    assert_eq!(
        s,
        vec!["10".to_string(), "11".to_string(), "12".to_string()]
            .into_iter()
            .collect()
    );
}
