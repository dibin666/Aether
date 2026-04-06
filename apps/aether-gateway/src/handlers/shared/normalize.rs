use std::collections::BTreeSet;

pub(crate) fn normalize_string_list(values: Option<Vec<String>>) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for value in values.into_iter().flatten() {
        let trimmed = value.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        out.push(trimmed.to_string());
    }
    (!out.is_empty()).then_some(out)
}

pub(crate) fn normalize_json_object(
    value: Option<serde_json::Value>,
    field_name: &str,
) -> Result<Option<serde_json::Value>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Object(map) if map.is_empty() => Ok(None),
        serde_json::Value::Object(map) => Ok(Some(serde_json::Value::Object(map))),
        _ => Err(format!("{field_name} 必须是 JSON 对象")),
    }
}

pub(crate) fn normalize_json_array(
    value: Option<serde_json::Value>,
    field_name: &str,
) -> Result<Option<serde_json::Value>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Array(items) if items.is_empty() => Ok(None),
        serde_json::Value::Array(items) => Ok(Some(serde_json::Value::Array(items))),
        _ => Err(format!("{field_name} 必须是 JSON 数组")),
    }
}
