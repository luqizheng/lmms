use notification_service::models::ScopeType;
use notification_service::recipient_resolver::expand_local;
use std::collections::HashMap;

#[test]
fn expand_dedupes_users_across_scopes() {
    let mut user_to_roles: HashMap<u64, Vec<u64>> = HashMap::new();
    user_to_roles.insert(10, vec![1]);
    user_to_roles.insert(11, vec![2]);
    let mut user_to_dept: HashMap<u64, u64> = HashMap::new();
    user_to_dept.insert(10, 100);
    user_to_dept.insert(12, 100);
    let scopes = vec![
        (ScopeType::User, 10),
        (ScopeType::Role, 1),
        (ScopeType::Dept, 100),
        (ScopeType::User, 11),
    ];
    let s = expand_local(&scopes, &user_to_roles, &user_to_dept);
    assert_eq!(s, vec![10, 11, 12].into_iter().collect());
}
