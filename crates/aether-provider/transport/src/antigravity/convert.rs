use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

const SKIP_THOUGHT_SIGNATURE_VALIDATOR: &str = "skip_thought_signature_validator";
const CLAUDE_DEFAULT_STOP_SEQUENCES: &[&str] = &[
    "<|user|>",
    "<|bot|>",
    "<|context_request|>",
    "<|endoftext|>",
    "<|end_of_turn|>",
];

/// Converts the public gcli2api-compatible Antigravity entry formats into the
/// Gemini request shape accepted by Google's private v1internal API.
///
/// This bridge is deliberately permissive: unknown content blocks are ignored
/// for OpenAI requests and serialized as text for Claude requests, matching the
/// proxy behavior instead of rejecting the whole request through Aether's
/// lossless cross-format contract.
pub fn convert_antigravity_entry_request_to_gemini(
    source_api_format: &str,
    request_body: &Value,
) -> Option<Value> {
    match aether_ai_formats::normalize_api_format_alias(source_api_format).as_str() {
        "openai:chat" => convert_openai_chat_to_gemini(request_body),
        "claude:messages" => convert_claude_messages_to_gemini(request_body),
        "gemini:generate_content" => request_body.as_object().cloned().map(Value::Object),
        _ => None,
    }
}

fn convert_openai_chat_to_gemini(request_body: &Value) -> Option<Value> {
    let request = request_body.as_object()?;
    let messages = request
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut system_parts = Vec::new();
    let mut tool_names_by_id = BTreeMap::new();
    for message in &messages {
        let Some(message) = message.as_object() else {
            continue;
        };
        if message.get("role").and_then(Value::as_str) == Some("system") {
            append_text_parts(message.get("content"), &mut system_parts);
        }
        let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) else {
            continue;
        };
        for tool_call in tool_calls {
            let Some(id) = tool_call.get("id").and_then(Value::as_str) else {
                continue;
            };
            let name = tool_call
                .pointer("/function/name")
                .and_then(Value::as_str)
                .unwrap_or("unknown_function");
            tool_names_by_id.insert(id.to_string(), name.to_string());
        }
    }

    let mut contents = Vec::new();
    let mut pending_tool_responses = Vec::new();
    for message in &messages {
        let Some(message) = message.as_object() else {
            continue;
        };
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user");
        if role == "system" {
            continue;
        }
        if role == "tool" {
            let tool_call_id = message
                .get("tool_call_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let name = message
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| tool_names_by_id.get(tool_call_id).map(String::as_str))
                .unwrap_or("unknown_function");
            pending_tool_responses.push(json!({
                "functionResponse": {
                    "id": tool_call_id,
                    "name": name,
                    "response": openai_tool_response_object(message.get("content")),
                }
            }));
            continue;
        }
        flush_openai_tool_responses(&mut contents, &mut pending_tool_responses);

        let gemini_role = if role == "assistant" { "model" } else { "user" };
        let mut parts = Vec::new();
        append_openai_content_parts(message.get("content"), &mut parts);
        if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                let Some(function) = tool_call.get("function").and_then(Value::as_object) else {
                    continue;
                };
                let name = function
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown_function");
                let arguments = function
                    .get("arguments")
                    .map(parse_json_or_clone)
                    .unwrap_or_else(|| json!({}));
                parts.push(json!({
                    "functionCall": {
                        "id": tool_call.get("id").and_then(Value::as_str).unwrap_or_default(),
                        "name": normalize_function_name(name),
                        "args": arguments,
                    },
                    "thoughtSignature": SKIP_THOUGHT_SIGNATURE_VALIDATOR,
                }));
            }
        }
        if !parts.is_empty() {
            contents.push(json!({"role": gemini_role, "parts": parts}));
        }
    }
    flush_openai_tool_responses(&mut contents, &mut pending_tool_responses);
    if contents.is_empty() {
        contents.push(json!({
            "role": "user",
            "parts": [{"text": "请根据系统指令回答。"}],
        }));
    }

    let mut output = Map::new();
    output.insert("contents".to_string(), Value::Array(contents));
    output.insert(
        "generationConfig".to_string(),
        Value::Object(openai_generation_config(request)),
    );
    if !system_parts.is_empty() {
        output.insert(
            "systemInstruction".to_string(),
            json!({"parts": system_parts}),
        );
    }
    if let Some(tools) = convert_openai_tools(request.get("tools")) {
        output.insert("tools".to_string(), tools);
    }
    if let Some(tool_config) = convert_openai_tool_choice(request.get("tool_choice")) {
        output.insert("toolConfig".to_string(), tool_config);
    }
    if let Some(size) = request.get("size").cloned() {
        output.insert("size".to_string(), size);
    }
    Some(Value::Object(output))
}

fn convert_claude_messages_to_gemini(request_body: &Value) -> Option<Value> {
    let request = request_body.as_object()?;
    let messages = request
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut tool_names_by_id = BTreeMap::new();
    for message in &messages {
        let Some(blocks) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for block in blocks {
            if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                continue;
            }
            if let (Some(id), Some(name)) = (
                block.get("id").and_then(Value::as_str),
                block.get("name").and_then(Value::as_str),
            ) {
                tool_names_by_id.insert(id.to_string(), name.to_string());
            }
        }
    }

    let mut contents = Vec::new();
    for message in &messages {
        let Some(message) = message.as_object() else {
            continue;
        };
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user");
        if role == "system" {
            continue;
        }
        let mut parts = Vec::new();
        append_claude_content_parts(message.get("content"), &tool_names_by_id, &mut parts);
        if !parts.is_empty() {
            contents.push(json!({
                "role": if matches!(role, "assistant" | "model") { "model" } else { "user" },
                "parts": parts,
            }));
        }
    }
    let contents = reorganize_claude_tool_messages(contents);

    let mut system_parts = Vec::new();
    append_text_parts(request.get("system"), &mut system_parts);
    for message in &messages {
        if message.get("role").and_then(Value::as_str) == Some("system") {
            append_text_parts(message.get("content"), &mut system_parts);
        }
    }

    let mut output = Map::new();
    output.insert("contents".to_string(), Value::Array(contents));
    output.insert(
        "generationConfig".to_string(),
        Value::Object(claude_generation_config(request)),
    );
    if !system_parts.is_empty() {
        output.insert(
            "systemInstruction".to_string(),
            json!({"parts": system_parts}),
        );
    }
    if let Some(tools) = convert_claude_tools(request.get("tools")) {
        output.insert("tools".to_string(), tools);
    }
    if let Some(tool_config) = convert_claude_tool_choice(request.get("tool_choice")) {
        output.insert("toolConfig".to_string(), tool_config);
    }
    if let Some(size) = request.get("size").cloned() {
        output.insert("size".to_string(), size);
    }
    Some(Value::Object(output))
}

fn append_openai_content_parts(content: Option<&Value>, parts: &mut Vec<Value>) {
    match content {
        Some(Value::String(text)) if !text.is_empty() => parts.push(json!({"text": text})),
        Some(Value::Array(items)) => {
            for item in items {
                let Some(item) = item.as_object() else {
                    if let Some(text) = item.as_str().filter(|text| !text.is_empty()) {
                        parts.push(json!({"text": text}));
                    }
                    continue;
                };
                match item.get("type").and_then(Value::as_str) {
                    Some("text" | "input_text") => {
                        if let Some(text) = item.get("text").and_then(Value::as_str) {
                            parts.push(json!({"text": text}));
                        }
                    }
                    Some("image_url" | "input_image") => {
                        let image_url = item.get("image_url").and_then(|value| {
                            value
                                .get("url")
                                .and_then(Value::as_str)
                                .or_else(|| value.as_str())
                        });
                        if let Some((mime_type, data)) = image_url.and_then(parse_data_url) {
                            parts.push(json!({
                                "inlineData": {"mimeType": mime_type, "data": data}
                            }));
                        }
                    }
                    _ => {}
                }
            }
        }
        Some(other) if !other.is_null() => parts.push(json!({"text": other.to_string()})),
        _ => {}
    }
}

fn append_claude_content_parts(
    content: Option<&Value>,
    tool_names_by_id: &BTreeMap<String, String>,
    parts: &mut Vec<Value>,
) {
    match content {
        Some(Value::String(text)) if !text.trim().is_empty() => parts.push(json!({"text": text})),
        Some(Value::Array(blocks)) => {
            for block in blocks {
                let Some(block) = block.as_object() else {
                    if let Some(text) = block.as_str().filter(|text| !text.trim().is_empty()) {
                        parts.push(json!({"text": text}));
                    }
                    continue;
                };
                match block.get("type").and_then(Value::as_str) {
                    Some("thinking" | "redacted_thinking") => {}
                    Some("text") => {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            if !text.trim().is_empty() {
                                parts.push(json!({"text": text}));
                            }
                        }
                    }
                    Some("image") => {
                        let source = block.get("source").and_then(Value::as_object);
                        if source
                            .and_then(|value| value.get("type"))
                            .and_then(Value::as_str)
                            == Some("base64")
                        {
                            parts.push(json!({
                                "inlineData": {
                                    "mimeType": source.and_then(|value| value.get("media_type")).and_then(Value::as_str).unwrap_or("image/png"),
                                    "data": source.and_then(|value| value.get("data")).and_then(Value::as_str).unwrap_or_default(),
                                }
                            }));
                        }
                    }
                    Some("tool_use") => {
                        parts.push(json!({
                            "functionCall": {
                                "id": block.get("id").and_then(Value::as_str).unwrap_or_default(),
                                "name": block.get("name").and_then(Value::as_str).unwrap_or("unknown_function"),
                                "args": block.get("input").cloned().unwrap_or_else(|| json!({})),
                            },
                            "thoughtSignature": SKIP_THOUGHT_SIGNATURE_VALIDATOR,
                        }));
                    }
                    Some("tool_result") => {
                        let id = block
                            .get("tool_use_id")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let name = block
                            .get("name")
                            .and_then(Value::as_str)
                            .or_else(|| tool_names_by_id.get(id).map(String::as_str))
                            .unwrap_or("unknown_function");
                        parts.push(json!({
                            "functionResponse": {
                                "id": id,
                                "name": name,
                                "response": {"output": extract_claude_tool_result(block.get("content"))},
                            }
                        }));
                    }
                    _ => parts.push(json!({"text": Value::Object(block.clone()).to_string()})),
                }
            }
        }
        Some(other) if !other.is_null() => parts.push(json!({"text": other.to_string()})),
        _ => {}
    }
}

fn append_text_parts(content: Option<&Value>, parts: &mut Vec<Value>) {
    match content {
        Some(Value::String(text)) if !text.trim().is_empty() => parts.push(json!({"text": text})),
        Some(Value::Array(items)) => {
            for item in items {
                if let Some(text) = item
                    .get("text")
                    .and_then(Value::as_str)
                    .or_else(|| item.as_str())
                    .filter(|text| !text.trim().is_empty())
                {
                    parts.push(json!({"text": text}));
                }
            }
        }
        _ => {}
    }
}

fn openai_generation_config(request: &Map<String, Value>) -> Map<String, Value> {
    let mut config = Map::new();
    copy_field(request, "temperature", &mut config, "temperature");
    copy_field(request, "top_p", &mut config, "topP");
    copy_field(request, "top_k", &mut config, "topK");
    if let Some(max_tokens) = request
        .get("max_completion_tokens")
        .or_else(|| request.get("max_tokens"))
        .cloned()
    {
        config.insert("maxOutputTokens".to_string(), max_tokens);
    }
    if let Some(stop) = request.get("stop") {
        config.insert(
            "stopSequences".to_string(),
            match stop {
                Value::String(_) => Value::Array(vec![stop.clone()]),
                other => other.clone(),
            },
        );
    }
    copy_field(
        request,
        "frequency_penalty",
        &mut config,
        "frequencyPenalty",
    );
    copy_field(request, "presence_penalty", &mut config, "presencePenalty");
    copy_field(request, "n", &mut config, "candidateCount");
    copy_field(request, "seed", &mut config, "seed");

    if let Some(response_format) = request.get("response_format").and_then(Value::as_object) {
        match response_format.get("type").and_then(Value::as_str) {
            Some("json_schema") => {
                if let Some(schema) = response_format
                    .get("json_schema")
                    .and_then(|value| value.get("schema"))
                    .cloned()
                {
                    config.insert("responseSchema".to_string(), schema);
                    config.insert(
                        "responseMimeType".to_string(),
                        Value::String("application/json".to_string()),
                    );
                }
            }
            Some("json_object") => {
                config.insert(
                    "responseMimeType".to_string(),
                    Value::String("application/json".to_string()),
                );
            }
            Some("text") => {
                config.insert(
                    "responseMimeType".to_string(),
                    Value::String("text/plain".to_string()),
                );
            }
            _ => {}
        }
    }
    config
}

fn claude_generation_config(request: &Map<String, Value>) -> Map<String, Value> {
    let mut config = Map::from_iter([
        ("topP".to_string(), Value::from(1)),
        ("candidateCount".to_string(), Value::from(1)),
        ("temperature".to_string(), Value::from(0.4)),
        (
            "stopSequences".to_string(),
            Value::Array(
                CLAUDE_DEFAULT_STOP_SEQUENCES
                    .iter()
                    .map(|value| Value::String((*value).to_string()))
                    .collect(),
            ),
        ),
    ]);
    copy_field(request, "temperature", &mut config, "temperature");
    copy_field(request, "top_p", &mut config, "topP");
    copy_field(request, "top_k", &mut config, "topK");
    copy_field(request, "max_tokens", &mut config, "maxOutputTokens");

    let mut plan_mode = false;
    if let Some(thinking) = request.get("thinking").and_then(Value::as_object) {
        match thinking.get("type").and_then(Value::as_str) {
            Some("enabled") => {
                plan_mode = true;
                config.insert(
                    "thinkingConfig".to_string(),
                    json!({
                        "thinkingBudget": thinking.get("budget_tokens").cloned().unwrap_or_else(|| Value::from(48_000)),
                        "includeThoughts": true,
                    }),
                );
            }
            Some("disabled") => {
                config.insert(
                    "thinkingConfig".to_string(),
                    json!({"includeThoughts": false}),
                );
            }
            _ => {}
        }
    }
    if let Some(extra) = request.get("stop_sequences").and_then(Value::as_array) {
        let stop_sequences = config
            .entry("stopSequences".to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(stop_sequences) = stop_sequences.as_array_mut() {
            stop_sequences.extend(extra.iter().map(|value| {
                Value::String(
                    value
                        .as_str()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| value.to_string()),
                )
            }));
        }
    } else if plan_mode {
        config.insert("stopSequences".to_string(), Value::Array(Vec::new()));
    }
    config
}

fn convert_openai_tools(tools: Option<&Value>) -> Option<Value> {
    let tools = tools?.as_array()?;
    let mut declarations = Vec::new();
    for tool in tools {
        if tool.get("type").and_then(Value::as_str) != Some("function") {
            continue;
        }
        let Some(function) = tool.get("function").and_then(Value::as_object) else {
            continue;
        };
        let mut declaration = Map::new();
        declaration.insert(
            "name".to_string(),
            Value::String(normalize_function_name(
                function
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("_unnamed_function"),
            )),
        );
        if let Some(description) = function.get("description").cloned() {
            declaration.insert("description".to_string(), description);
        }
        if let Some(parameters) = function.get("parameters").cloned() {
            declaration.insert("parametersJsonSchema".to_string(), parameters);
        }
        declarations.push(Value::Object(declaration));
    }
    (!declarations.is_empty()).then(|| json!([{"functionDeclarations": declarations}]))
}

fn convert_claude_tools(tools: Option<&Value>) -> Option<Value> {
    let tools = tools?.as_array()?;
    let converted = tools
        .iter()
        .filter_map(|tool| {
            let name = tool.get("name").and_then(Value::as_str)?;
            Some(json!({
                "functionDeclarations": [{
                    "name": name,
                    "description": tool.get("description").cloned().unwrap_or_else(|| Value::String(String::new())),
                    "parametersJsonSchema": tool.get("input_schema").cloned().unwrap_or_else(|| json!({})),
                }]
            }))
        })
        .collect::<Vec<_>>();
    (!converted.is_empty()).then(|| Value::Array(converted))
}

fn convert_openai_tool_choice(tool_choice: Option<&Value>) -> Option<Value> {
    let tool_choice = tool_choice?;
    let config = match tool_choice {
        Value::String(choice) => match choice.as_str() {
            "none" => json!({"functionCallingConfig": {"mode": "NONE"}}),
            "required" => json!({"functionCallingConfig": {"mode": "ANY"}}),
            _ => json!({"functionCallingConfig": {"mode": "AUTO"}}),
        },
        Value::Object(choice) if choice.get("type").and_then(Value::as_str) == Some("function") => {
            let name = choice
                .get("function")
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str)?;
            json!({
                "functionCallingConfig": {
                    "mode": "ANY",
                    "allowedFunctionNames": [name],
                }
            })
        }
        _ => json!({"functionCallingConfig": {"mode": "AUTO"}}),
    };
    Some(config)
}

fn convert_claude_tool_choice(tool_choice: Option<&Value>) -> Option<Value> {
    let choice = tool_choice?.as_object()?;
    let config = match choice.get("type").and_then(Value::as_str) {
        Some("auto") => json!({"functionCallingConfig": {"mode": "AUTO"}}),
        Some("any") => json!({"functionCallingConfig": {"mode": "ANY"}}),
        Some("tool") => {
            let name = choice.get("name").and_then(Value::as_str)?;
            json!({
                "functionCallingConfig": {
                    "mode": "ANY",
                    "allowedFunctionNames": [name],
                }
            })
        }
        _ => return None,
    };
    Some(config)
}

fn reorganize_claude_tool_messages(contents: Vec<Value>) -> Vec<Value> {
    let mut responses_by_id = BTreeMap::new();
    for content in &contents {
        let Some(parts) = content.get("parts").and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            if let Some(id) = part.pointer("/functionResponse/id").and_then(Value::as_str) {
                responses_by_id.insert(id.to_string(), part.clone());
            }
        }
    }

    let mut result = Vec::new();
    for content in contents {
        let role = content
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user");
        let Some(parts) = content.get("parts").and_then(Value::as_array) else {
            result.push(content);
            continue;
        };
        for part in parts {
            if part.get("functionResponse").is_some() {
                continue;
            }
            if let Some(id) = part.pointer("/functionCall/id").and_then(Value::as_str) {
                result.push(json!({"role": "model", "parts": [part.clone()]}));
                if let Some(response) = responses_by_id.get(id) {
                    result.push(json!({"role": "user", "parts": [response.clone()]}));
                }
            } else {
                result.push(json!({"role": role, "parts": [part.clone()]}));
            }
        }
    }
    result
}

fn flush_openai_tool_responses(contents: &mut Vec<Value>, pending: &mut Vec<Value>) {
    if pending.is_empty() {
        return;
    }
    contents.push(json!({"role": "user", "parts": std::mem::take(pending)}));
}

fn openai_tool_response_object(content: Option<&Value>) -> Value {
    let parsed = content
        .map(parse_json_or_clone)
        .unwrap_or_else(|| Value::String(String::new()));
    if parsed.is_object() {
        parsed
    } else {
        json!({"result": parsed})
    }
}

fn extract_claude_tool_result(content: Option<&Value>) -> String {
    match content {
        Some(Value::Array(items)) => items
            .first()
            .and_then(|item| {
                item.get("text")
                    .and_then(Value::as_str)
                    .or_else(|| item.as_str())
            })
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| items.first().map(Value::to_string).unwrap_or_default()),
        Some(Value::String(text)) => text.clone(),
        Some(value) => value.to_string(),
        None => String::new(),
    }
}

fn parse_json_or_clone(value: &Value) -> Value {
    value
        .as_str()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_else(|| value.clone())
}

fn parse_data_url(value: &str) -> Option<(&str, &str)> {
    let (metadata, data) = value.strip_prefix("data:")?.split_once(',')?;
    let mime_type = metadata.strip_suffix(";base64")?;
    Some((mime_type, data))
}

fn normalize_function_name(name: &str) -> String {
    let normalized = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | ':') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if normalized.is_empty() {
        "_unnamed_function".to_string()
    } else {
        normalized
    }
}

fn copy_field(source: &Map<String, Value>, from: &str, target: &mut Map<String, Value>, to: &str) {
    if let Some(value) = source.get(from).cloned() {
        target.insert(to.to_string(), value);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::convert_antigravity_entry_request_to_gemini;

    #[test]
    fn converts_openai_messages_tools_and_generation_config_per_gcli_contract() {
        let converted = convert_antigravity_entry_request_to_gemini(
            "openai:chat",
            &json!({
                "model": "claude-sonnet-4-6",
                "messages": [
                    {"role": "system", "content": "be concise"},
                    {"role": "user", "content": [{"type": "text", "text": "weather"}]},
                    {"role": "assistant", "content": null, "tool_calls": [{
                        "id": "toolu_1",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": "{\"city\":\"HZ\"}"}
                    }]},
                    {"role": "tool", "tool_call_id": "toolu_1", "content": "{\"ok\":true}"}
                ],
                "max_tokens": 1024,
                "tools": [{
                    "type": "function",
                    "function": {"name": "get_weather", "parameters": {"type": "object"}}
                }],
                "tool_choice": "required"
            }),
        )
        .expect("openai conversion");

        assert_eq!(
            converted["systemInstruction"]["parts"][0]["text"],
            "be concise"
        );
        assert_eq!(
            converted["contents"][1]["parts"][0]["functionCall"]["id"],
            "toolu_1"
        );
        assert_eq!(
            converted["contents"][2]["parts"][0]["functionResponse"]["id"],
            "toolu_1"
        );
        assert_eq!(converted["generationConfig"]["maxOutputTokens"], 1024);
        assert_eq!(
            converted["toolConfig"]["functionCallingConfig"]["mode"],
            "ANY"
        );
    }

    #[test]
    fn converts_claude_blocks_without_rejecting_thinking_history() {
        let converted = convert_antigravity_entry_request_to_gemini(
            "claude:messages",
            &json!({
                "model": "claude-sonnet-4-6",
                "system": [{"type": "text", "text": "system"}],
                "messages": [
                    {"role": "assistant", "content": [
                        {"type": "thinking", "thinking": "old", "signature": "stale"},
                        {"type": "tool_use", "id": "toolu_2", "name": "lookup", "input": {"q": "x"}}
                    ]},
                    {"role": "user", "content": [
                        {"type": "tool_result", "tool_use_id": "toolu_2", "content": "done"}
                    ]}
                ],
                "max_tokens": 2048,
                "thinking": {"type": "enabled", "budget_tokens": 1000}
            }),
        )
        .expect("claude conversion");

        assert_eq!(converted["systemInstruction"]["parts"][0]["text"], "system");
        assert_eq!(
            converted["contents"][0]["parts"][0]["functionCall"]["id"],
            "toolu_2"
        );
        assert_eq!(
            converted["contents"][1]["parts"][0]["functionResponse"]["id"],
            "toolu_2"
        );
        assert_eq!(
            converted["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            1000
        );
    }
}
