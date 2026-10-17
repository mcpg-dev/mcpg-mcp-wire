//! Whether a change to an event type's schemas keeps its subscribers whole.
//!
//! A server that changes `inputSchema` or `payloadSchema` in a way a
//! subscriber cannot absorb has to say so (`-32014 schema_changed`); one
//! that only describes more need not. A server and a client that compare
//! two descriptors read the same rule here.

use serde_json::{Map, Value};

/// Keywords a schema may change without breaking a subscriber: what it
/// describes, not what it accepts.
const ANNOTATIONS: [&str; 5] = ["properties", "required", "description", "title", "examples"];

/// Which side of a subscription a schema describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaRole {
    /// What a subscriber sends (`inputSchema`): it may come to require
    /// less, never more.
    Input,
    /// What a subscriber receives (`payloadSchema`): it may come to
    /// guarantee more, never less.
    Payload,
}

/// Whether moving from `old` to `new` keeps every subscriber whole: every
/// property stays as it was, `required` moves only the way `role` allows,
/// and nothing else that constrains a value changes. Anything else is a
/// break.
pub fn is_additive_change(old: &Value, new: &Value, role: SchemaRole) -> bool {
    if old == new {
        return true;
    }
    let (Some(old), Some(new)) = (old.as_object(), new.as_object()) else {
        return false;
    };
    let constrains = |key: &String| !ANNOTATIONS.contains(&key.as_str());
    if old
        .iter()
        .filter(|(k, _)| constrains(k))
        .any(|(k, v)| new.get(k) != Some(v))
        || new
            .iter()
            .filter(|(k, _)| constrains(k))
            .any(|(k, v)| old.get(k) != Some(v))
    {
        return false;
    }
    let properties = |schema: &Map<String, Value>| -> Map<String, Value> {
        schema
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    };
    let (old_props, new_props) = (properties(old), properties(new));
    if old_props.iter().any(|(k, v)| new_props.get(k) != Some(v)) {
        return false;
    }
    let required = |schema: &Map<String, Value>| -> Vec<String> {
        schema
            .get("required")
            .and_then(Value::as_array)
            .map(|r| {
                r.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let (old_required, new_required) = (required(old), required(new));
    match role {
        SchemaRole::Input => new_required.iter().all(|r| old_required.contains(r)),
        SchemaRole::Payload => old_required.iter().all(|r| new_required.contains(r)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_schema_may_describe_more_but_never_accept_or_promise_less() {
        let base = json!({
            "type": "object",
            "required": ["id"],
            "properties": { "id": { "type": "string" } },
        });
        let with = |patch: Value| {
            let mut schema = base.clone();
            for (k, v) in patch.as_object().unwrap() {
                schema[k] = v.clone();
            }
            schema
        };
        let more_properties = with(json!({
            "properties": { "id": { "type": "string" }, "region": { "type": "string" } },
            "description": "Orders.",
        }));
        let retyped = with(json!({ "properties": { "id": { "type": "integer" } } }));
        let more_required = with(json!({ "required": ["id", "region"] }));
        let less_required = with(json!({ "required": [] }));
        let closed = with(json!({ "additionalProperties": false }));

        for role in [SchemaRole::Input, SchemaRole::Payload] {
            assert!(is_additive_change(&base, &base, role));
            assert!(
                is_additive_change(&base, &more_properties, role),
                "{role:?}"
            );
            assert!(!is_additive_change(&base, &retyped, role), "{role:?}");
            assert!(!is_additive_change(&base, &closed, role), "{role:?}");
            assert!(!is_additive_change(&base, &json!(true), role), "{role:?}");
        }
        assert!(!is_additive_change(
            &base,
            &more_required,
            SchemaRole::Input
        ));
        assert!(is_additive_change(&base, &less_required, SchemaRole::Input));
        assert!(is_additive_change(
            &base,
            &more_required,
            SchemaRole::Payload
        ));
        assert!(!is_additive_change(
            &base,
            &less_required,
            SchemaRole::Payload
        ));
    }
}
