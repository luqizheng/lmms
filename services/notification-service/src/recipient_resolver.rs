use crate::models::ScopeType;
use std::collections::HashSet;

pub fn expand_local(
    scopes: &[(ScopeType, u64)],
    user_to_roles: &std::collections::HashMap<u64, Vec<u64>>,
    user_to_dept: &std::collections::HashMap<u64, u64>,
) -> HashSet<u64> {
    let mut by_role: std::collections::HashMap<u64, Vec<u64>> = std::collections::HashMap::new();
    for (u, rs) in user_to_roles {
        for r in rs {
            by_role.entry(*r).or_default().push(*u);
        }
    }
    let mut by_dept: std::collections::HashMap<u64, Vec<u64>> = std::collections::HashMap::new();
    for (u, d) in user_to_dept {
        by_dept.entry(*d).or_default().push(*u);
    }
    let mut out = HashSet::new();
    for (t, id) in scopes {
        match t {
            ScopeType::User => {
                out.insert(*id);
            }
            ScopeType::Role => {
                if let Some(v) = by_role.get(id) {
                    out.extend(v);
                }
            }
            ScopeType::Dept => {
                if let Some(v) = by_dept.get(id) {
                    out.extend(v);
                }
            }
        }
    }
    out
}
