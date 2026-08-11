use serde_json::{Map, Value};
use uuid::Uuid;

use super::auth::AntigravityRequestAuth;
use super::normalize::normalize_antigravity_cli_inner_request;
use super::profile::{
    current_antigravity_compatibility_profile, ANTIGRAVITY_GOOGLE_ONE_AI_CREDIT_TYPE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntigravityEnvelopeRequestType {
    Agent,
    Checkpoint,
    EndpointTest,
}

impl AntigravityEnvelopeRequestType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Checkpoint => "checkpoint",
            Self::EndpointTest => "endpoint_test",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AntigravityRequestEnvelopeSupport {
    Supported(Value),
    Unsupported(AntigravityRequestEnvelopeUnsupportedReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AntigravityRequestEnvelopeUnsupportedReason {
    NonObjectBody,
    MissingContents,
    MissingRequestId,
    MissingModel,
}

pub fn classify_antigravity_safe_request_body(
    request_body: &Value,
) -> Result<(), AntigravityRequestEnvelopeUnsupportedReason> {
    let Value::Object(map) = request_body else {
        return Err(AntigravityRequestEnvelopeUnsupportedReason::NonObjectBody);
    };
    if !map.contains_key("contents") && existing_v1internal_request_object(map).is_none() {
        return Err(AntigravityRequestEnvelopeUnsupportedReason::MissingContents);
    }

    Ok(())
}

pub fn build_antigravity_safe_v1internal_request(
    auth: &AntigravityRequestAuth,
    request_id: &str,
    model: &str,
    request_body: &Value,
    request_type: AntigravityEnvelopeRequestType,
) -> AntigravityRequestEnvelopeSupport {
    if request_id.trim().is_empty() {
        return AntigravityRequestEnvelopeSupport::Unsupported(
            AntigravityRequestEnvelopeUnsupportedReason::MissingRequestId,
        );
    }
    if model.trim().is_empty() {
        return AntigravityRequestEnvelopeSupport::Unsupported(
            AntigravityRequestEnvelopeUnsupportedReason::MissingModel,
        );
    }
    if let Err(reason) = classify_antigravity_safe_request_body(request_body) {
        return AntigravityRequestEnvelopeSupport::Unsupported(reason);
    }

    let Value::Object(source) = request_body else {
        return AntigravityRequestEnvelopeSupport::Unsupported(
            AntigravityRequestEnvelopeUnsupportedReason::NonObjectBody,
        );
    };

    let mut inner_request: Map<String, Value> = existing_v1internal_request_object(source)
        .cloned()
        .unwrap_or_else(|| source.clone());
    let effective_request_type = existing_v1internal_request_type(source).unwrap_or(request_type);
    let raw_request_id = non_empty_string_field(source, "requestId").unwrap_or(request_id);
    let effective_request_id =
        normalize_antigravity_request_id(raw_request_id, effective_request_type);
    normalize_antigravity_cli_inner_request(
        &mut inner_request,
        effective_request_id.as_str(),
        model,
        effective_request_type == AntigravityEnvelopeRequestType::Agent,
    );

    let profile = current_antigravity_compatibility_profile();
    let mut envelope = Map::from_iter([
        (
            "project".to_string(),
            Value::String(auth.project_id.clone()),
        ),
        ("requestId".to_string(), Value::String(effective_request_id)),
        ("request".to_string(), Value::Object(inner_request)),
        ("model".to_string(), Value::String(model.to_string())),
        (
            "userAgent".to_string(),
            Value::String(profile.envelope_user_agent.to_string()),
        ),
        (
            "requestType".to_string(),
            Value::String(effective_request_type.as_str().to_string()),
        ),
    ]);
    if auth.enable_google_one_ai_credit {
        envelope.insert(
            "enabledCreditTypes".to_string(),
            Value::Array(vec![Value::String(
                ANTIGRAVITY_GOOGLE_ONE_AI_CREDIT_TYPE.to_string(),
            )]),
        );
    }

    AntigravityRequestEnvelopeSupport::Supported(Value::Object(envelope))
}

fn normalize_antigravity_request_id(
    request_id: &str,
    request_type: AntigravityEnvelopeRequestType,
) -> String {
    let request_id = request_id.trim();
    if request_type != AntigravityEnvelopeRequestType::Agent || request_id.starts_with("agent/") {
        return request_id.to_string();
    }
    let stable_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, request_id.as_bytes());
    format!("agent/{stable_id}")
}

fn existing_v1internal_request_object(source: &Map<String, Value>) -> Option<&Map<String, Value>> {
    source
        .get("request")
        .and_then(Value::as_object)
        .filter(|request| request.contains_key("contents"))
}

fn non_empty_string_field<'a>(source: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    source
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn existing_v1internal_request_type(
    source: &Map<String, Value>,
) -> Option<AntigravityEnvelopeRequestType> {
    match non_empty_string_field(source, "requestType")? {
        "agent" => Some(AntigravityEnvelopeRequestType::Agent),
        "checkpoint" => Some(AntigravityEnvelopeRequestType::Checkpoint),
        "endpoint_test" => Some(AntigravityEnvelopeRequestType::EndpointTest),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_antigravity_safe_v1internal_request, classify_antigravity_safe_request_body,
        AntigravityEnvelopeRequestType, AntigravityRequestAuth, AntigravityRequestEnvelopeSupport,
    };
    use crate::antigravity::ANTIGRAVITY_ENVELOPE_USER_AGENT;

    fn sample_auth() -> AntigravityRequestAuth {
        AntigravityRequestAuth {
            project_id: "project-ant-123".to_string(),
            client_version: None,
            session_id: None,
            enable_google_one_ai_credit: false,
        }
    }

    #[test]
    fn real_agent_request_preserves_antigravity_agent_fields() {
        let request_body = json!({
            "model": "client-side-model-should-not-be-nested",
            "contents": [
                {
                    "role": "user",
                    "parts": [
                        { "text": "Reply with OK only." }
                    ]
                }
            ],
            "systemInstruction": {
                "role": "user",
                "parts": [
                    { "text": "Antigravity agent system prompt" }
                ]
            },
            "generationConfig": {
                "maxOutputTokens": 8192,
                "thinkingConfig": {
                    "includeThoughts": true,
                    "thinkingBudget": 4000
                }
            },
            "toolConfig": {
                "functionCallingConfig": {
                    "mode": "VALIDATED"
                }
            },
            "tools": [
                {
                    "functionDeclarations": [
                        {
                            "name": "run_command",
                            "description": "Run a command",
                            "parameters": {
                                "type": "object",
                                "properties": {
                                    "cmd": { "type": "string" }
                                },
                                "required": ["cmd"]
                            }
                        }
                    ]
                }
            ],
            "labels": {
                "trajectory_id": "trajectory-123",
                "used_claude": "false"
            },
            "sessionId": "session-ant-123",
            "safetySettings": [
                { "category": "HARM_CATEGORY_UNSPECIFIED" }
            ]
        });

        assert_eq!(
            classify_antigravity_safe_request_body(&request_body),
            Ok(())
        );

        let envelope = match build_antigravity_safe_v1internal_request(
            &sample_auth(),
            "request-ant-agent-123",
            "gemini-3.5-flash-low",
            &request_body,
            AntigravityEnvelopeRequestType::Agent,
        ) {
            AntigravityRequestEnvelopeSupport::Supported(envelope) => envelope,
            AntigravityRequestEnvelopeSupport::Unsupported(reason) => {
                panic!("real agent envelope should be supported: {reason:?}")
            }
        };

        assert_eq!(envelope["project"], "project-ant-123");
        assert!(envelope["requestId"]
            .as_str()
            .is_some_and(|value| value.starts_with("agent/")));
        assert_eq!(envelope["model"], "gemini-3.5-flash-low");
        assert_eq!(envelope["userAgent"], ANTIGRAVITY_ENVELOPE_USER_AGENT);
        assert_eq!(envelope["requestType"], "agent");
        assert!(envelope["request"].get("model").is_none());
        assert!(envelope["request"].get("safetySettings").is_none());
        assert_eq!(
            envelope["request"]["systemInstruction"]["parts"][0]["text"],
            "Antigravity agent system prompt"
        );
        assert!(envelope["request"]["generationConfig"]
            .get("thinkingConfig")
            .is_none());
        assert_eq!(
            envelope["request"]["toolConfig"]["functionCallingConfig"]["mode"],
            "VALIDATED"
        );
        assert_eq!(
            envelope["request"]["tools"][0]["functionDeclarations"][0]["name"],
            "run_command"
        );
        assert_eq!(
            envelope["request"]["labels"]["trajectory_id"],
            "session-ant-123"
        );
        assert_eq!(envelope["request"]["sessionId"], "session-ant-123");
    }

    #[test]
    fn checkpoint_request_type_builds_checkpoint_envelope() {
        let request_body = json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [
                        { "text": "checkpoint context" }
                    ]
                }
            ],
            "generationConfig": {
                "maxOutputTokens": 8192,
                "thinkingConfig": {
                    "includeThoughts": true,
                    "thinkingBudget": 4000
                }
            },
            "toolConfig": {
                "functionCallingConfig": {
                    "mode": "NONE"
                }
            }
        });

        let envelope = match build_antigravity_safe_v1internal_request(
            &sample_auth(),
            "request-ant-checkpoint-123",
            "gemini-3.5-flash-low",
            &request_body,
            AntigravityEnvelopeRequestType::Checkpoint,
        ) {
            AntigravityRequestEnvelopeSupport::Supported(envelope) => envelope,
            AntigravityRequestEnvelopeSupport::Unsupported(reason) => {
                panic!("checkpoint envelope should be supported: {reason:?}")
            }
        };

        assert_eq!(envelope["requestType"], "checkpoint");
        assert_eq!(
            envelope["request"]["toolConfig"]["functionCallingConfig"]["mode"],
            "NONE"
        );
    }

    #[test]
    fn existing_v1internal_envelope_is_not_double_wrapped() {
        let request_body = json!({
            "project": "client-side-project",
            "requestId": "client-request-id-123",
            "model": "gemini-3.5-flash-low",
            "userAgent": "antigravity",
            "requestType": "checkpoint",
            "request": {
                "contents": [
                    {
                        "role": "user",
                        "parts": [
                            { "text": "checkpoint context" }
                        ]
                    }
                ],
                "generationConfig": {
                    "thinkingConfig": {
                        "includeThoughts": true
                    }
                },
                "toolConfig": {
                    "functionCallingConfig": {
                        "mode": "NONE"
                    }
                }
            }
        });

        assert_eq!(
            classify_antigravity_safe_request_body(&request_body),
            Ok(())
        );

        let envelope = match build_antigravity_safe_v1internal_request(
            &sample_auth(),
            "trace-request-id-should-not-overwrite-client-id",
            "mapped-antigravity-model",
            &request_body,
            AntigravityEnvelopeRequestType::Agent,
        ) {
            AntigravityRequestEnvelopeSupport::Supported(envelope) => envelope,
            AntigravityRequestEnvelopeSupport::Unsupported(reason) => {
                panic!("existing v1internal envelope should be supported: {reason:?}")
            }
        };

        assert_eq!(envelope["project"], "project-ant-123");
        assert_eq!(envelope["requestId"], "client-request-id-123");
        assert_eq!(envelope["model"], "mapped-antigravity-model");
        assert_eq!(envelope["userAgent"], "antigravity");
        assert_eq!(envelope["requestType"], "checkpoint");
        assert!(envelope["request"].get("request").is_none());
        assert_eq!(
            envelope["request"]["contents"][0]["parts"][0]["text"],
            "checkpoint context"
        );
        assert_eq!(
            envelope["request"]["toolConfig"]["functionCallingConfig"]["mode"],
            "NONE"
        );
    }

    #[test]
    fn google_one_ai_credit_is_only_emitted_after_explicit_opt_in() {
        let request_body = json!({
            "contents": [{"role": "user", "parts": [{"text": "hello"}]}]
        });
        let without_credit = match build_antigravity_safe_v1internal_request(
            &sample_auth(),
            "request-without-credit",
            "gemini-3-flash-agent",
            &request_body,
            AntigravityEnvelopeRequestType::Agent,
        ) {
            AntigravityRequestEnvelopeSupport::Supported(envelope) => envelope,
            AntigravityRequestEnvelopeSupport::Unsupported(reason) => {
                panic!("request should be supported: {reason:?}")
            }
        };
        assert!(without_credit.get("enabledCreditTypes").is_none());

        let mut auth = sample_auth();
        auth.enable_google_one_ai_credit = true;
        let with_credit = match build_antigravity_safe_v1internal_request(
            &auth,
            "request-with-credit",
            "gemini-3-flash-agent",
            &request_body,
            AntigravityEnvelopeRequestType::Agent,
        ) {
            AntigravityRequestEnvelopeSupport::Supported(envelope) => envelope,
            AntigravityRequestEnvelopeSupport::Unsupported(reason) => {
                panic!("request should be supported: {reason:?}")
            }
        };
        assert_eq!(with_credit["enabledCreditTypes"], json!(["GOOGLE_ONE_AI"]));
    }
}
