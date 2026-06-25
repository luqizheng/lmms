use serde_json::Value;

/// Recursively strip menu-management internal fields (currently `_menu`) from
/// every route node's `meta` block. Mutates in place.
pub fn strip_internal_fields(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items.iter_mut() {
                strip_internal_fields(item);
            }
        }
        Value::Object(map) => {
            if let Some(meta) = map.get_mut("meta") {
                if let Some(meta_obj) = meta.as_object_mut() {
                    meta_obj.remove("_menu");
                }
            }
            if let Some(children) = map.get_mut("children") {
                strip_internal_fields(children);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_top_level_menu_field() {
        let mut v = json!([
            { "path": "/a", "meta": { "title": "A", "_menu": { "id": 1 } } }
        ]);
        strip_internal_fields(&mut v);
        assert_eq!(
            v,
            json!([{ "path": "/a", "meta": { "title": "A" } }])
        );
    }

    #[test]
    fn strips_nested_menu_field() {
        let mut v = json!([
            {
                "path": "/a",
                "meta": { "title": "A", "_menu": { "id": 1 } },
                "children": [
                    {
                        "path": "/a/b",
                        "meta": { "title": "B", "_menu": { "id": 2 } }
                    }
                ]
            }
        ]);
        strip_internal_fields(&mut v);
        assert_eq!(
            v,
            json!([
                {
                    "path": "/a",
                    "meta": { "title": "A" },
                    "children": [
                        { "path": "/a/b", "meta": { "title": "B" } }
                    ]
                }
            ])
        );
    }

    #[test]
    fn nodes_without_meta_are_untouched() {
        let mut v = json!([{ "path": "/a" }]);
        let original = v.clone();
        strip_internal_fields(&mut v);
        assert_eq!(v, original);
    }

    #[test]
    fn unrelated_meta_keys_are_preserved() {
        let mut v = json!([
            { "path": "/a", "meta": { "title": "A", "roles": ["admin"], "_menu": { "id": 1 } } }
        ]);
        strip_internal_fields(&mut v);
        assert_eq!(
            v,
            json!([{ "path": "/a", "meta": { "title": "A", "roles": ["admin"] } }])
        );
    }
}
