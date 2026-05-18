use serde_json::Value;

/// Deep merge two JSON values, with `right` taking priority over `left`.
///
/// Rules:
/// - Objects: recursively merge keys
/// - Arrays: concatenate and deduplicate (for string arrays)
/// - Null in right: deletes the key from left
/// - All other types: right replaces left
pub fn deep_merge(left: &Value, right: &Value) -> Value {
    match (left, right) {
        (Value::Object(l), Value::Object(r)) => {
            let mut merged = l.clone();
            for (key, r_val) in r {
                if let Some(l_val) = l.get(key) {
                    // Null in right deletes the key
                    if r_val.is_null() {
                        merged.remove(key);
                    } else {
                        merged.insert(key.clone(), deep_merge(l_val, r_val));
                    }
                } else {
                    if r_val.is_null() {
                        // Skip null values for new keys
                        continue;
                    }
                    merged.insert(key.clone(), r_val.clone());
                }
            }
            Value::Object(merged)
        }
        (Value::Array(l), Value::Array(r)) => {
            // Concatenate arrays, deduplicate if they are strings
            let mut merged = l.clone();
            for r_val in r {
                if r_val.is_null() {
                    continue;
                }
                // Deduplicate string arrays
                if let (Some(r_str), true) = (r_val.as_str(), is_string_array(l)) {
                    if !l.iter().any(|v| v.as_str() == Some(r_str)) {
                        merged.push(r_val.clone());
                    }
                } else {
                    merged.push(r_val.clone());
                }
            }
            Value::Array(merged)
        }
        (_, Value::Null) => {
            // Null in right deletes left
            Value::Null
        }
        (_, r) => r.clone(),
    }
}

/// Check if an array contains only strings.
fn is_string_array(arr: &[Value]) -> bool {
    !arr.is_empty() && arr.iter().all(|v| v.is_string())
}

/// Merge multiple settings values in order (later values take priority).
pub fn merge_all(values: &[Value]) -> Value {
    values
        .iter()
        .fold(Value::Object(serde_json::Map::new()), |acc, v| {
            deep_merge(&acc, v)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deep_merge_objects() {
        let left = serde_json::json!({
            "a": 1,
            "b": { "c": 2, "d": 3 },
            "e": "hello"
        });
        let right = serde_json::json!({
            "b": { "c": 10, "f": 4 },
            "g": "world"
        });

        let result = deep_merge(&left, &right);
        assert_eq!(result["a"], 1);
        assert_eq!(result["b"]["c"], 10);
        assert_eq!(result["b"]["d"], 3);
        assert_eq!(result["b"]["f"], 4);
        assert_eq!(result["e"], "hello");
        assert_eq!(result["g"], "world");
    }

    #[test]
    fn test_deep_merge_null_deletes() {
        let left = serde_json::json!({
            "a": 1,
            "b": 2
        });
        let right = serde_json::json!({
            "b": null
        });

        let result = deep_merge(&left, &right);
        assert_eq!(result["a"], 1);
        assert!(result.get("b").is_none());
    }

    #[test]
    fn test_deep_merge_arrays_concat() {
        let left = serde_json::json!(["a", "b"]);
        let right = serde_json::json!(["b", "c"]);

        let result = deep_merge(&left, &right);
        assert_eq!(result.as_array().unwrap().len(), 3);
        assert!(result.as_array().unwrap().contains(&serde_json::json!("a")));
        assert!(result.as_array().unwrap().contains(&serde_json::json!("b")));
        assert!(result.as_array().unwrap().contains(&serde_json::json!("c")));
    }
}
