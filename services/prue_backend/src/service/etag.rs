use serde_json::{Map, Value};

/// Compute a stable SHA-256 etag over canonicalized JSON.
///
/// Object keys are sorted recursively so that two JSON values with the same
/// semantic content always hash to the same etag, regardless of the original
/// key order produced by the client.
pub fn compute(value: &Value) -> String {
    let canonical = canonicalize(value);
    let bytes = serde_json::to_vec(&canonical).expect("serde_json must serialize canonical Value");
    crate::db::sha256_hex(&bytes)
}

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted: Vec<(&String, &Value)> = map.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(b.0));
            let mut out = Map::with_capacity(map.len());
            for (k, v) in sorted {
                out.insert(k.clone(), canonicalize(v));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn same_value_yields_same_etag() {
        let a = json!([{ "path": "/a", "meta": { "title": "A" } }]);
        let b = json!([{ "path": "/a", "meta": { "title": "A" } }]);
        assert_eq!(compute(&a), compute(&b));
    }

    #[test]
    fn key_order_does_not_change_etag() {
        let a = json!({ "a": 1, "b": 2, "c": { "x": 1, "y": 2 } });
        let b = json!({ "c": { "y": 2, "x": 1 }, "b": 2, "a": 1 });
        assert_eq!(compute(&a), compute(&b));
    }

    #[test]
    fn different_value_yields_different_etag() {
        let a = json!([{ "path": "/a" }]);
        let b = json!([{ "path": "/b" }]);
        assert_ne!(compute(&a), compute(&b));
    }
}
