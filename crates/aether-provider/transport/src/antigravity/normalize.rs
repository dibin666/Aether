use std::collections::{BTreeMap, VecDeque};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const SKIP_THOUGHT_SIGNATURE_VALIDATOR: &str = "skip_thought_signature_validator";

pub(crate) fn normalize_antigravity_cli_inner_request(
    inner_request: &mut Map<String, Value>,
    request_id: &str,
    model: &str,
    is_agent_request: bool,
) {
    inner_request.remove("model");
    inner_request.remove("safetySettings");
    inner_request.remove("safety_settings");

    normalize_generation_config(inner_request, model);
    normalize_tools(inner_request);
    normalize_contents(inner_request, request_id, model);

    if is_agent_request {
        let session_id = ensure_session_id(inner_request, request_id);
        ensure_agent_labels(inner_request, model, session_id.as_str());
        force_validated_tool_mode(inner_request);
    }
}

fn normalize_generation_config(inner_request: &mut Map<String, Value>, model: &str) {
    let contains_tool_exchange = request_contains_tool_exchange(inner_request);
    let generation_key = if inner_request.contains_key("generation_config")
        && !inner_request.contains_key("generationConfig")
    {
        "generation_config"
    } else {
        "generationConfig"
    };
    let Some(generation_config) = inner_request
        .get_mut(generation_key)
        .and_then(Value::as_object_mut)
    else {
        return;
    };

    for unsupported in [
        "presencePenalty",
        "presence_penalty",
        "frequencyPenalty",
        "frequency_penalty",
        "stopSequences",
        "stop_sequences",
    ] {
        generation_config.remove(unsupported);
    }

    let normalized_model = model.trim().to_ascii_lowercase();
    if normalized_model.contains("gemini-3") {
        generation_config.remove("thinkingConfig");
        generation_config.remove("thinking_config");
    } else if normalized_model.contains("claude") && contains_tool_exchange {
        generation_config.remove("thinkingConfig");
        generation_config.remove("thinking_config");
    }
}

fn normalize_tools(inner_request: &mut Map<String, Value>) {
    let Some(tools) = inner_request.get_mut("tools").and_then(Value::as_array_mut) else {
        return;
    };

    for tool in tools {
        let Some(tool_object) = tool.as_object_mut() else {
            continue;
        };

        if let Some(custom) = tool_object
            .remove("custom")
            .and_then(|value| value.as_object().cloned())
        {
            let mut declaration = Map::new();
            declaration.insert(
                "name".to_string(),
                custom
                    .get("name")
                    .cloned()
                    .unwrap_or_else(|| Value::String(String::new())),
            );
            if let Some(description) = custom.get("description").cloned() {
                declaration.insert("description".to_string(), description);
            }
            let mut schema = custom
                .get("input_schema")
                .or_else(|| custom.get("inputSchema"))
                .cloned()
                .unwrap_or_else(empty_object_schema);
            let schema_root = schema.clone();
            inline_local_schema_refs(&mut schema, &schema_root, 0);
            sanitize_tool_schema(&mut schema);
            declaration.insert("parameters".to_string(), schema);
            tool_object.insert(
                "functionDeclarations".to_string(),
                Value::Array(vec![Value::Object(declaration)]),
            );
        }

        let declarations = tool_object
            .remove("function_declarations")
            .or_else(|| tool_object.remove("functionDeclarations"));
        let Some(mut declarations) = declarations else {
            continue;
        };
        let Some(declarations_array) = declarations.as_array_mut() else {
            continue;
        };

        for declaration in declarations_array {
            let Some(declaration_object) = declaration.as_object_mut() else {
                continue;
            };
            let mut schema = declaration_object
                .remove("parameters")
                .or_else(|| declaration_object.remove("parametersJsonSchema"))
                .or_else(|| declaration_object.remove("parameters_json_schema"))
                .unwrap_or_else(empty_object_schema);
            let schema_root = schema.clone();
            inline_local_schema_refs(&mut schema, &schema_root, 0);
            sanitize_tool_schema(&mut schema);
            declaration_object.insert("parameters".to_string(), schema);
        }
        tool_object.insert("functionDeclarations".to_string(), declarations);
    }
}

fn sanitize_tool_schema(schema: &mut Value) {
    match schema {
        Value::Array(items) => {
            for item in items {
                sanitize_tool_schema(item);
            }
        }
        Value::Object(object) => {
            collapse_schema_union(object, "oneOf");
            collapse_schema_union(object, "anyOf");
            normalize_schema_type(object);

            for value in object.values_mut() {
                sanitize_tool_schema(value);
            }

            for unsupported in [
                "title",
                "$schema",
                "$id",
                "$ref",
                "ref",
                "$defs",
                "definitions",
                "allOf",
                "oneOf",
                "anyOf",
                "additionalProperties",
                "patternProperties",
                "dependencies",
                "propertyNames",
                "if",
                "then",
                "else",
                "contains",
                "additionalItems",
                "examples",
                "example",
                "readOnly",
                "writeOnly",
                "nullable",
                "strict",
                "default",
                "minLength",
                "maxLength",
                "minimum",
                "maximum",
                "exclusiveMinimum",
                "exclusiveMaximum",
                "minItems",
                "maxItems",
                "pattern",
                "format",
                "uniqueItems",
            ] {
                object.remove(unsupported);
            }
            object.retain(|key, _| !key.starts_with("x-"));

            if object.contains_key("properties") && !object.contains_key("type") {
                object.insert("type".to_string(), Value::String("object".to_string()));
            }
            if object.get("type").and_then(Value::as_str) == Some("object") {
                object
                    .entry("properties".to_string())
                    .or_insert_with(|| Value::Object(Map::new()));
            }
            retain_known_required_properties(object);
        }
        _ => {}
    }
}

fn inline_local_schema_refs(schema: &mut Value, root: &Value, depth: usize) {
    if depth >= 16 {
        if let Some(object) = schema.as_object_mut() {
            object.remove("$ref");
            object.remove("ref");
        }
        return;
    }
    match schema {
        Value::Array(items) => {
            for item in items {
                inline_local_schema_refs(item, root, depth + 1);
            }
        }
        Value::Object(object) => {
            let reference = object
                .get("$ref")
                .or_else(|| object.get("ref"))
                .and_then(Value::as_str)
                .filter(|reference| reference.starts_with("#/"))
                .map(ToOwned::to_owned);
            if let Some(reference) = reference {
                if let Some(Value::Object(resolved)) = root.pointer(&reference[1..]).cloned() {
                    let siblings = object
                        .iter()
                        .filter(|(key, _)| key.as_str() != "$ref" && key.as_str() != "ref")
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect::<Map<_, _>>();
                    *object = resolved;
                    object.extend(siblings);
                }
            }
            object.remove("$ref");
            object.remove("ref");
            for value in object.values_mut() {
                inline_local_schema_refs(value, root, depth + 1);
            }
        }
        _ => {}
    }
}

fn collapse_schema_union(object: &mut Map<String, Value>, key: &str) {
    let Some(variants) = object
        .remove(key)
        .and_then(|value| value.as_array().cloned())
    else {
        return;
    };
    if variants.is_empty() {
        return;
    }

    let const_values = variants
        .iter()
        .filter_map(|variant| variant.get("const").cloned())
        .collect::<Vec<_>>();
    if const_values.len() == variants.len() {
        object.insert("type".to_string(), Value::String("string".to_string()));
        object.insert("enum".to_string(), Value::Array(const_values));
        return;
    }

    let preferred = variants.into_iter().find(|variant| {
        variant
            .get("type")
            .and_then(Value::as_str)
            .is_none_or(|schema_type| schema_type != "null")
    });
    let Some(Value::Object(preferred)) = preferred else {
        return;
    };
    for (preferred_key, preferred_value) in preferred {
        object.entry(preferred_key).or_insert(preferred_value);
    }
}

fn normalize_schema_type(object: &mut Map<String, Value>) {
    let Some(Value::Array(types)) = object.get("type") else {
        return;
    };
    let normalized = types
        .iter()
        .filter_map(Value::as_str)
        .find(|schema_type| !schema_type.eq_ignore_ascii_case("null"))
        .unwrap_or("string")
        .to_ascii_lowercase();
    object.insert("type".to_string(), Value::String(normalized));
}

fn retain_known_required_properties(object: &mut Map<String, Value>) {
    let Some(properties) = object.get("properties").and_then(Value::as_object) else {
        return;
    };
    let property_names = properties
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let Some(required) = object.get_mut("required").and_then(Value::as_array_mut) else {
        return;
    };
    required.retain(|value| {
        value
            .as_str()
            .is_some_and(|name| property_names.contains(name))
    });
    if required.is_empty() {
        object.remove("required");
    }
}

fn normalize_contents(inner_request: &mut Map<String, Value>, request_id: &str, model: &str) {
    let normalized_model = model.trim().to_ascii_lowercase();
    let claude_model = normalized_model.contains("claude");
    let Some(contents) = inner_request
        .get_mut("contents")
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    if claude_model {
        while contents
            .last()
            .is_some_and(|content| content.get("role").and_then(Value::as_str) == Some("model"))
        {
            contents.pop();
        }
    }

    let mut pending_ids_by_name: BTreeMap<String, VecDeque<String>> = BTreeMap::new();
    let mut call_index = 0usize;
    for content in contents {
        let Some(parts) = content.get_mut("parts").and_then(Value::as_array_mut) else {
            continue;
        };
        parts.retain(is_non_empty_part);

        for part in parts {
            let Some(part_object) = part.as_object_mut() else {
                continue;
            };
            normalize_text_part(part_object);

            if !claude_model && part_requires_thought_signature(part_object) {
                part_object.remove("thought_signature");
                part_object
                    .entry("thoughtSignature".to_string())
                    .or_insert_with(|| Value::String(SKIP_THOUGHT_SIGNATURE_VALIDATOR.to_string()));
            }

            if !claude_model {
                continue;
            }
            let function_call_key = if part_object.contains_key("functionCall") {
                "functionCall"
            } else {
                "function_call"
            };
            if let Some(function_call) = part_object
                .get_mut(function_call_key)
                .and_then(Value::as_object_mut)
            {
                let name = function_call
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool")
                    .to_string();
                let call_id = function_call
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| stable_tool_call_id(request_id, name.as_str(), call_index));
                function_call.insert("id".to_string(), Value::String(call_id.clone()));
                pending_ids_by_name
                    .entry(name)
                    .or_default()
                    .push_back(call_id);
                call_index += 1;
                continue;
            }
            let function_response_key = if part_object.contains_key("functionResponse") {
                "functionResponse"
            } else {
                "function_response"
            };
            if let Some(function_response) = part_object
                .get_mut(function_response_key)
                .and_then(Value::as_object_mut)
            {
                if function_response
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
                {
                    continue;
                }
                let name = function_response
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool");
                let response_id = pending_ids_by_name
                    .get_mut(name)
                    .and_then(VecDeque::pop_front)
                    .unwrap_or_else(|| stable_tool_call_id(request_id, name, call_index));
                function_response.insert("id".to_string(), Value::String(response_id));
                call_index += 1;
            }
        }
    }
}

fn is_non_empty_part(part: &Value) -> bool {
    part.as_object().is_some_and(|object| {
        object
            .iter()
            .any(|(key, value)| key == "thought" || !is_empty_json_value(value))
    })
}

fn is_empty_json_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(value) => value.is_empty(),
        Value::Array(value) => value.is_empty(),
        Value::Object(value) => value.is_empty(),
        _ => false,
    }
}

fn normalize_text_part(part: &mut Map<String, Value>) {
    let Some(text) = part.get_mut("text") else {
        return;
    };
    match text {
        Value::String(value) => {
            *value = value.trim_end().to_string();
        }
        Value::Array(items) => {
            let normalized = items
                .iter()
                .filter_map(|item| {
                    item.as_str().map(ToOwned::to_owned).or_else(|| {
                        item.get("text")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    })
                })
                .collect::<Vec<_>>()
                .join(" ");
            *text = Value::String(normalized);
        }
        other => {
            *other = Value::String(other.to_string());
        }
    }
}

fn part_requires_thought_signature(part: &Map<String, Value>) -> bool {
    part.contains_key("functionCall")
        || part.contains_key("function_call")
        || part.get("thought").and_then(Value::as_bool) == Some(true)
        || part.contains_key("thoughtSignature")
        || part.contains_key("thought_signature")
}

fn request_contains_tool_exchange(inner_request: &Map<String, Value>) -> bool {
    inner_request
        .get("contents")
        .and_then(Value::as_array)
        .is_some_and(|contents| {
            contents.iter().any(|content| {
                content
                    .get("parts")
                    .and_then(Value::as_array)
                    .is_some_and(|parts| {
                        parts.iter().any(|part| {
                            part.get("functionCall").is_some()
                                || part.get("function_call").is_some()
                                || part.get("functionResponse").is_some()
                                || part.get("function_response").is_some()
                        })
                    })
            })
        })
}

fn ensure_session_id(inner_request: &mut Map<String, Value>, request_id: &str) -> String {
    if let Some(session_id) = inner_request
        .get("sessionId")
        .or_else(|| inner_request.get("session_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return session_id.to_string();
    }

    let seed = first_user_text(inner_request).unwrap_or(request_id);
    let digest = Sha256::digest(seed.as_bytes());
    let numeric = u64::from_be_bytes(
        digest[..8]
            .try_into()
            .expect("sha256 prefix is eight bytes"),
    ) & i64::MAX as u64;
    let session_id = format!("-{numeric}");
    inner_request.insert("sessionId".to_string(), Value::String(session_id.clone()));
    inner_request.remove("session_id");
    session_id
}

fn first_user_text(inner_request: &Map<String, Value>) -> Option<&str> {
    inner_request
        .get("contents")
        .and_then(Value::as_array)?
        .iter()
        .filter(|content| content.get("role").and_then(Value::as_str) == Some("user"))
        .filter_map(|content| content.get("parts").and_then(Value::as_array))
        .flatten()
        .find_map(|part| {
            part.get("text")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
        })
}

fn ensure_agent_labels(inner_request: &mut Map<String, Value>, model: &str, session_id: &str) {
    let used_claude = model.trim().to_ascii_lowercase().contains("claude");
    let labels = inner_request
        .entry("labels".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !labels.is_object() {
        *labels = Value::Object(Map::new());
    }
    let labels = labels
        .as_object_mut()
        .expect("labels was normalized to object");
    labels.insert(
        "last_step_index".to_string(),
        Value::String("1".to_string()),
    );
    labels.insert("model_enum".to_string(), Value::String(model.to_string()));
    labels.insert(
        "trajectory_id".to_string(),
        Value::String(session_id.to_string()),
    );
    labels.insert(
        "used_claude".to_string(),
        Value::String(used_claude.to_string()),
    );
    labels.insert(
        "used_claude_conservative".to_string(),
        Value::String(used_claude.to_string()),
    );
}

fn force_validated_tool_mode(inner_request: &mut Map<String, Value>) {
    let tool_config = inner_request
        .entry("toolConfig".to_string())
        .or_insert_with(|| json!({}));
    if !tool_config.is_object() {
        *tool_config = json!({});
    }
    let tool_config = tool_config
        .as_object_mut()
        .expect("toolConfig was normalized to object");
    let function_config = tool_config
        .entry("functionCallingConfig".to_string())
        .or_insert_with(|| json!({}));
    if !function_config.is_object() {
        *function_config = json!({});
    }
    function_config
        .as_object_mut()
        .expect("functionCallingConfig was normalized to object")
        .insert("mode".to_string(), Value::String("VALIDATED".to_string()));
}

fn stable_tool_call_id(request_id: &str, name: &str, index: usize) -> String {
    let seed = format!("{request_id}:{name}:{index}");
    format!(
        "toolu_{}",
        Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes()).simple()
    )
}

fn empty_object_schema() -> Value {
    json!({"type": "object", "properties": {}})
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::normalize_antigravity_cli_inner_request;

    #[test]
    fn normalizes_agent_identity_and_tool_schema() {
        let mut body = json!({
            "contents": [{"role": "user", "parts": [{"text": "hello"}]}],
            "tools": [{
                "function_declarations": [{
                    "name": "lookup",
                    "parametersJsonSchema": {
                        "type": "object",
                        "properties": {
                            "query": {"type": ["string", "null"], "minLength": 1}
                        },
                        "required": ["query", "missing"],
                        "additionalProperties": false
                    }
                }]
            }]
        });
        let object = body.as_object_mut().expect("body is object");

        normalize_antigravity_cli_inner_request(object, "trace-1", "gemini-3-flash-agent", true);

        assert!(object["sessionId"].as_str().is_some());
        assert_eq!(object["labels"]["model_enum"], "gemini-3-flash-agent");
        assert_eq!(
            object["toolConfig"]["functionCallingConfig"]["mode"],
            "VALIDATED"
        );
        let schema = &object["tools"][0]["functionDeclarations"][0]["parameters"];
        assert_eq!(schema["properties"]["query"]["type"], "string");
        assert!(schema["properties"]["query"].get("minLength").is_none());
        assert!(schema.get("additionalProperties").is_none());
        assert_eq!(schema["required"], json!(["query"]));
    }

    #[test]
    fn claude_tool_exchange_receives_paired_ids_and_drops_conflicting_thinking() {
        let mut body = json!({
            "contents": [{
                "role": "model",
                "parts": [{"functionCall": {"name": "lookup", "args": {"q": "x"}}}]
            }, {
                "role": "user",
                "parts": [{"functionResponse": {"name": "lookup", "response": {"ok": true}}}]
            }],
            "generationConfig": {
                "thinkingConfig": {"includeThoughts": true, "thinkingBudget": 1024}
            }
        });
        let object = body.as_object_mut().expect("body is object");

        normalize_antigravity_cli_inner_request(object, "trace-2", "claude-sonnet-4-6", true);

        let call_id = object["contents"][0]["parts"][0]["functionCall"]["id"]
            .as_str()
            .expect("call id");
        assert!(call_id.starts_with("toolu_"));
        assert_eq!(
            object["contents"][1]["parts"][0]["functionResponse"]["id"],
            call_id
        );
        assert!(object["generationConfig"].get("thinkingConfig").is_none());
    }
}
