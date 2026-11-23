//! Translate declared DTO fields, leaving user-controlled dictionary keys intact.
use super::{internal_protocol, invalid, snake_name};
use rmcp::ErrorData;
use serde_json::Value;

fn core_name(field: &str) -> String {
    let mut value = String::new();
    let mut capitalize = false;
    for ch in field.chars() {
        if ch == '_' {
            capitalize = true;
        } else {
            value.push(if capitalize {
                ch.to_ascii_uppercase()
            } else {
                ch
            });
            capitalize = false;
        }
    }
    value
}

fn alternatives<'a>(
    schema: &'a Value,
    defs: &'a Value,
    out: &mut Vec<&'a Value>,
) -> Result<(), ErrorData> {
    if out.iter().any(|s| std::ptr::eq(*s, schema)) {
        return Ok(());
    }
    out.push(schema);
    if let Some(link) = schema.get("$ref").and_then(Value::as_str) {
        let name = link
            .strip_prefix("#/$defs/")
            .ok_or_else(internal_protocol)?;
        alternatives(defs.get(name).ok_or_else(internal_protocol)?, defs, out)?;
    }
    for field in ["allOf", "oneOf", "anyOf"] {
        if let Some(parts) = schema[field].as_array() {
            for part in parts {
                alternatives(part, defs, out)?;
            }
        }
    }
    Ok(())
}

fn map(value: Value, schema: &Value, defs: &Value, incoming: bool) -> Result<Value, ErrorData> {
    let mut shapes = Vec::new();
    alternatives(schema, defs, &mut shapes)?;
    match value {
        Value::Object(fields) => {
            let mut converted = serde_json::Map::new();
            for (name, child) in fields {
                let wire_name = if incoming {
                    name.clone()
                } else {
                    snake_name(&name)
                };
                if let Some(field) = shapes
                    .iter()
                    .find_map(|part| part["properties"].get(&wire_name))
                {
                    let dest = if incoming {
                        core_name(&name)
                    } else {
                        wire_name
                    };
                    if converted
                        .insert(dest, map(child, field, defs, incoming)?)
                        .is_some()
                    {
                        return Err(invalid());
                    }
                } else {
                    let additional = shapes.iter().find_map(|s| s.get("additionalProperties"));
                    if incoming && additional == Some(&Value::Bool(false)) {
                        return Err(invalid());
                    }
                    let child = match additional {
                        Some(rule) if rule.is_object() => map(child, rule, defs, incoming)?,
                        _ => child,
                    };
                    converted.insert(name, child);
                }
            }
            Ok(Value::Object(converted))
        }
        Value::Array(items) => {
            if let Some(rule) = shapes.iter().find_map(|s| s.get("items")) {
                items
                    .into_iter()
                    .map(|item| map(item, rule, defs, incoming))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::Array)
            } else {
                Ok(Value::Array(items))
            }
        }
        Value::Null if incoming => {
            let nullable = shapes.iter().any(|s| {
                let ty = &s["type"];
                ty == "null"
                    || ty
                        .as_array()
                        .is_some_and(|items| items.iter().any(|t| t == "null"))
                    || s["enum"]
                        .as_array()
                        .is_some_and(|items| items.contains(&Value::Null))
                    || s.get("const") == Some(&Value::Null)
                    || s.as_object().is_some_and(|s| s.is_empty())
            });
            if nullable {
                Ok(Value::Null)
            } else {
                Err(invalid())
            }
        }
        value => Ok(value),
    }
}

pub(super) fn to_core(value: Value, schema: &Value) -> Result<Value, ErrorData> {
    map(value, schema, &schema["$defs"], true)
}

pub(super) fn from_core(data: Value, schema: &Value) -> Result<Value, ErrorData> {
    map(data, &schema["properties"]["data"], &schema["$defs"], false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn preserves_header_names_and_distinguishes_omitted_null_and_false() {
        let schema = json!({"type":"object","additionalProperties":false,"properties":{
            "before_repository_id":{"anyOf":[{"type":"string"},{"type":"null"}]},
            "keep_existing":{"type":"boolean"},"headers":{"additionalProperties":{"type":"string"}}
        }});
        assert_eq!(
            to_core(
                json!({"before_repository_id":null,"keep_existing":false,
            "headers":{"X-Repository-ID":"opaque"}}),
                &schema
            )
            .expect("convert explicit nullable and nested fields"),
            json!({"beforeRepositoryId":null,
            "keepExisting":false,"headers":{"X-Repository-ID":"opaque"}})
        );
        assert_eq!(
            to_core(json!({}), &schema).expect("preserve omitted fields"),
            json!({})
        );
        assert!(to_core(json!({"keep_existing":null}), &schema).is_err());
    }
}
