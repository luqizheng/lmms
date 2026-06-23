use crate::models::ScopeType;
use std::collections::HashSet;

pub fn expand_local(
    scopes: &[(ScopeType, String)],
    user_to_roles: &std::collections::HashMap<String, Vec<String>>,
    user_to_dept: &std::collections::HashMap<String, String>,
) -> HashSet<String> {
    let mut by_role: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (u, rs) in user_to_roles {
        for r in rs {
            by_role.entry(r.clone()).or_default().push(u.clone());
        }
    }
    let mut by_dept: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (u, d) in user_to_dept {
        by_dept.entry(d.clone()).or_default().push(u.clone());
    }
    let mut out = HashSet::new();
    for (t, id) in scopes {
        match t {
            ScopeType::User => {
                out.insert(id.clone());
            }
            ScopeType::Role => {
                if let Some(v) = by_role.get(id) {
                    out.extend(v.iter().cloned());
                }
            }
            ScopeType::Dept => {
                if let Some(v) = by_dept.get(id) {
                    out.extend(v.iter().cloned());
                }
            }
        }
    }
    out
}
