use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::formats::{id::FormatId, FormatContext, FormatError};
use crate::protocol::canonical::{
    canonical_usage_to_openai_responses_usage, canonical_usage_total_input_tokens,
    openai_usage_to_canonical, CanonicalRequest, CanonicalResponse,
};

const EXTENSION: &str = "decisions";
pub const REQUEST_METADATA: &str = "decisions_questions";

#[derive(Clone, Serialize, Deserialize)]
pub struct DecisionOption {
    pub value: Value,
    pub description: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DecisionQuestion {
    pub key: String,
    pub name: Option<String>,
    pub kind: String,
    pub instructions: String,
    pub options: Vec<DecisionOption>,
    pub criteria: Option<Value>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DecisionsRequest {
    pub input: Value,
    pub questions: Vec<DecisionQuestion>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DecisionAnswer {
    pub key: String,
    pub name: Option<String>,
    pub kind: String,
    pub value: Value,
    pub probabilities: Vec<Value>,
    pub confidence: Option<Value>,
}

pub fn is_decisions(format: FormatId) -> bool {
    matches!(
        format,
        FormatId::GeminiDecisions | FormatId::OpenAiDecisions | FormatId::SystemOneDecisions
    )
}

fn invalid(format: FormatId, field: &str, reason: &str) -> FormatError {
    FormatError::UnsupportedField {
        format: format.as_str().to_string(),
        field: field.to_string(),
        reason: reason.to_string(),
    }
}

fn key(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn text(value: &Value, format: FormatId, field: &str) -> Result<String, FormatError> {
    value.as_str().map(str::to_string).ok_or_else(|| {
        invalid(
            format,
            field,
            "only text is supported for cross-format conversion; unsupported modality",
        )
    })
}

fn input(value: &Value, format: FormatId) -> Result<Value, FormatError> {
    if value.is_string() || (format == FormatId::SystemOneDecisions && value.is_object()) {
        return Ok(value.clone());
    }
    let block_type = if format == FormatId::GeminiDecisions {
        "text"
    } else {
        "input_text"
    };
    if let Some(blocks) = value.as_array() {
        let mut result = String::new();
        for block in blocks {
            if block["type"] != block_type {
                return Err(invalid(format, "input", "unsupported modality"));
            }
            result.push_str(&text(&block["text"], format, "input.text")?);
        }
        return Ok(json!(result));
    }
    if value["type"] == block_type {
        return Ok(json!(text(&value["text"], format, "input.text")?));
    }
    if format == FormatId::OpenAiDecisions && value.is_object() && value.get("type").is_none() {
        return Ok(value.clone());
    }
    Err(invalid(
        format,
        "input",
        "unsupported modality or input shape",
    ))
}

pub fn parse_request(
    format: FormatId,
    body: &Value,
    ctx: &FormatContext,
) -> Result<CanonicalRequest, FormatError> {
    if ctx.upstream_is_stream || body.get("stream").and_then(Value::as_bool) == Some(true) {
        return Err(invalid(
            format,
            "stream",
            "Decisions conversion is sync-only",
        ));
    }
    let object = body
        .as_object()
        .ok_or_else(|| invalid(format, "body", "expected an object"))?;
    for field in object.keys() {
        if !matches!(
            field.as_str(),
            "model" | "input" | "state" | "questions" | "stream"
        ) {
            return Err(invalid(
                format,
                field,
                "field has no cross-format equivalent",
            ));
        }
    }
    let input_field = if format == FormatId::SystemOneDecisions {
        "state"
    } else {
        "input"
    };
    let input = input(&body[input_field], format)?;
    if input.as_str() == Some("") {
        return Err(invalid(format, input_field, "input cannot be empty"));
    }
    if object.contains_key(if input_field == "state" {
        "input"
    } else {
        "state"
    }) {
        return Err(invalid(
            format,
            input_field,
            "unexpected context field for this dialect",
        ));
    }
    let entries: Vec<(Option<String>, &Value)> = if format == FormatId::OpenAiDecisions {
        body["questions"]
            .as_array()
            .ok_or_else(|| invalid(format, "questions", "expected an array"))?
            .iter()
            .map(|q| (q.get("name").and_then(Value::as_str).map(str::to_string), q))
            .collect()
    } else {
        body["questions"]
            .as_object()
            .ok_or_else(|| invalid(format, "questions", "expected a map"))?
            .iter()
            .map(|(k, v)| (Some(k.clone()), v))
            .collect()
    };
    if entries.is_empty() {
        return Err(invalid(
            format,
            "questions",
            "at least one question is required",
        ));
    }
    let mut names = BTreeSet::new();
    for (name, _) in &entries {
        if let Some(name) = name {
            if !names.insert(name.clone()) {
                return Err(invalid(
                    format,
                    "questions.name",
                    "duplicate question names cannot be converted",
                ));
            }
        }
    }
    let mut questions = Vec::new();
    for (index, (name, question)) in entries.into_iter().enumerate() {
        let mut question_key = name.clone().unwrap_or_else(|| format!("_question_{index}"));
        if name.is_none() {
            while names.contains(&question_key) {
                question_key.push('_');
            }
            names.insert(question_key.clone());
        }
        let q = question
            .as_object()
            .ok_or_else(|| invalid(format, "questions", "expected a question object"))?;
        for field in q.keys() {
            if !matches!(
                field.as_str(),
                "name" | "type" | "instructions" | "criteria" | "choices" | "levels"
            ) {
                return Err(invalid(
                    format,
                    field,
                    "question field has no cross-format equivalent",
                ));
            }
        }
        let kind = match question["type"].as_str() {
            Some("predicate" | "noul") => "binary",
            Some("choice") => "choice",
            Some("score") => "score",
            _ => {
                return Err(invalid(
                    format,
                    "questions.type",
                    "unsupported question type",
                ))
            }
        }
        .to_string();
        if (format == FormatId::OpenAiDecisions && question["type"] == "noul")
            || (format != FormatId::OpenAiDecisions && question["type"] == "predicate")
        {
            return Err(invalid(
                format,
                "questions.type",
                "question type does not belong to this dialect",
            ));
        }
        let instructions = text(&question["instructions"], format, "questions.instructions")?;
        let mut options = Vec::new();
        let mut criteria = None;
        if kind == "binary" {
            if let Some(c) = question.get("criteria") {
                if format == FormatId::OpenAiDecisions {
                    return Err(invalid(
                        format,
                        "criteria",
                        "predicate does not accept criteria",
                    ));
                }
                let (yes, no) = if format == FormatId::GeminiDecisions {
                    ("true_description", "false_description")
                } else {
                    ("true", "false")
                };
                criteria = Some(
                    json!({"true":text(&c[yes],format,"criteria")?,"false":text(&c[no],format,"criteria")?}),
                );
            }
        } else if format == FormatId::OpenAiDecisions {
            let field = if kind == "choice" {
                "choices"
            } else {
                "levels"
            };
            for (i, option) in question[field]
                .as_array()
                .ok_or_else(|| invalid(format, field, "expected an array"))?
                .iter()
                .enumerate()
            {
                let value = if kind == "choice" {
                    option["value"].clone()
                } else {
                    json!(i)
                };
                let description = if kind == "choice" {
                    option
                        .get("description")
                        .map(|v| text(v, format, field))
                        .transpose()?
                        .unwrap_or_default()
                } else {
                    let label = text(&option["label"], format, field)?;
                    if option
                        .get("description")
                        .and_then(Value::as_str)
                        .is_some_and(|d| d != label)
                    {
                        return Err(invalid(
                            format,
                            field,
                            "distinct score label and description cannot map losslessly",
                        ));
                    }
                    label
                };
                options.push(DecisionOption { value, description });
            }
        } else if kind == "choice" {
            for (value, description) in question["criteria"]
                .as_object()
                .ok_or_else(|| invalid(format, "criteria", "expected a choice map"))?
            {
                options.push(DecisionOption {
                    value: json!(value),
                    description: text(description, format, "criteria")?,
                });
            }
        } else {
            for (i, description) in question["criteria"]
                .as_array()
                .ok_or_else(|| invalid(format, "criteria", "expected ordered levels"))?
                .iter()
                .enumerate()
            {
                options.push(DecisionOption {
                    value: json!(i),
                    description: text(description, format, "criteria")?,
                });
            }
        }
        if kind != "binary" {
            let maximum = if kind == "score" { 10 } else { 208 };
            if !(2..=maximum).contains(&options.len()) {
                return Err(invalid(format, "criteria", "unsupported option count"));
            }
            let mut values = BTreeSet::new();
            for option in &options {
                let k = if kind == "score" {
                    option.value.to_string()
                } else {
                    key(&option.value).ok_or_else(|| {
                        invalid(format, "choices.value", "expected string or boolean")
                    })?
                };
                if !values.insert(k) {
                    return Err(invalid(
                        format,
                        "choices.value",
                        "choice values collide when stringified",
                    ));
                }
            }
        }
        questions.push(DecisionQuestion {
            key: question_key,
            name,
            kind,
            instructions,
            options,
            criteria,
        });
    }
    let decisions = DecisionsRequest { input, questions };
    Ok(CanonicalRequest {
        model: text(&body["model"], format, "model")?,
        extensions: BTreeMap::from([(EXTENSION.to_string(), json!(decisions))]),
        ..Default::default()
    })
}

pub fn request_metadata(request: &CanonicalRequest) -> Value {
    let Some(value) = request.extensions.get(EXTENSION) else {
        return Value::Null;
    };
    let Ok(mut request) = serde_json::from_value::<DecisionsRequest>(value.clone()) else {
        return Value::Null;
    };
    for q in &mut request.questions {
        q.instructions.clear();
        q.criteria = None;
    }
    json!(request.questions)
}

pub fn emit_request(
    format: FormatId,
    request: &CanonicalRequest,
    ctx: &FormatContext,
) -> Result<Value, FormatError> {
    if ctx.upstream_is_stream {
        return Err(invalid(
            format,
            "stream",
            "Decisions conversion is sync-only",
        ));
    }
    let decisions: DecisionsRequest = serde_json::from_value(
        request
            .extensions
            .get(EXTENSION)
            .cloned()
            .unwrap_or(Value::Null),
    )
    .map_err(|_| invalid(format, "questions", "missing canonical Decisions request"))?;
    let mut questions = Map::new();
    let mut array = Vec::new();
    for q in decisions.questions {
        let mut value = json!({"instructions":q.instructions,"type":if q.kind == "binary" {if format == FormatId::OpenAiDecisions {"predicate"} else {"noul"}} else {&q.kind}});
        if q.kind == "binary" {
            if let Some(criteria) = q.criteria {
                if format == FormatId::OpenAiDecisions {
                    value["instructions"] = json!(format!(
                        "{}\nTrue: {}\nFalse: {}",
                        q.instructions,
                        criteria["true"].as_str().unwrap_or_default(),
                        criteria["false"].as_str().unwrap_or_default()
                    ));
                } else if format == FormatId::GeminiDecisions {
                    value["criteria"] = json!({"true_description":criteria["true"],"false_description":criteria["false"]});
                } else {
                    value["criteria"] = criteria;
                }
            }
        } else if format == FormatId::OpenAiDecisions {
            value[if q.kind == "choice" {
                "choices"
            } else {
                "levels"
            }] = Value::Array(
                q.options
                    .iter()
                    .map(|o| {
                        if q.kind == "choice" {
                            json!({"value":o.value,"description":o.description})
                        } else {
                            json!({"label":o.description})
                        }
                    })
                    .collect(),
            );
        } else if q.kind == "choice" {
            value["criteria"] = Value::Object(
                q.options
                    .iter()
                    .map(|o| (key(&o.value).unwrap_or_default(), json!(o.description)))
                    .collect(),
            );
        } else {
            value["criteria"] = json!(q.options.iter().map(|o| &o.description).collect::<Vec<_>>());
        }
        if format == FormatId::OpenAiDecisions {
            value["name"] = json!(q.key);
            array.push(value);
        } else {
            questions.insert(q.key, value);
        }
    }
    let input_field = if format == FormatId::SystemOneDecisions {
        "state"
    } else {
        "input"
    };
    let input = if format == FormatId::GeminiDecisions && !decisions.input.is_string() {
        json!(decisions.input.to_string())
    } else {
        decisions.input
    };
    Ok(
        json!({"model":ctx.mapped_model_or(&request.model), input_field:input,"questions":if format == FormatId::OpenAiDecisions {Value::Array(array)} else {Value::Object(questions)}}),
    )
}

fn metadata(ctx: &FormatContext) -> Vec<DecisionQuestion> {
    ctx.report_context
        .as_ref()
        .and_then(|v| v.get(REQUEST_METADATA))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

fn probability(value: &Value, format: FormatId) -> Result<(), FormatError> {
    if value.as_f64().is_some_and(|p| (0.0..=1.0).contains(&p)) {
        Ok(())
    } else {
        Err(invalid(
            format,
            "answers.probability",
            "expected a probability between 0 and 1",
        ))
    }
}

pub fn parse_response(
    format: FormatId,
    body: &Value,
    ctx: &FormatContext,
) -> Result<CanonicalResponse, FormatError> {
    let questions = metadata(ctx);
    let entries: Vec<(String, &Value)> = if format == FormatId::OpenAiDecisions {
        body["answers"]
            .as_array()
            .ok_or_else(|| invalid(format, "answers", "expected an array"))?
            .iter()
            .enumerate()
            .map(|(i, a)| {
                (
                    a["name"].as_str().map(str::to_string).unwrap_or_else(|| {
                        questions
                            .get(i)
                            .map(|q| q.key.clone())
                            .unwrap_or_else(|| format!("_question_{i}"))
                    }),
                    a,
                )
            })
            .collect()
    } else {
        body["answers"]
            .as_object()
            .ok_or_else(|| invalid(format, "answers", "expected a map"))?
            .iter()
            .map(|(k, v)| (k.clone(), v))
            .collect()
    };
    if entries.is_empty() {
        return Err(invalid(format, "answers", "empty answers"));
    }
    if !questions.is_empty() && entries.len() != questions.len() {
        return Err(invalid(
            format,
            "answers",
            "answer count does not match request",
        ));
    }
    let mut answers = Vec::new();
    let mut seen = BTreeSet::new();
    for (key, a) in entries {
        if !seen.insert(key.clone()) {
            return Err(invalid(format, "answers.name", "duplicate answer names"));
        }
        let q = questions.iter().find(|q| q.key == key);
        if !questions.is_empty() && q.is_none() {
            return Err(invalid(format, "answers", "unknown question key"));
        }
        let kind = match a["type"].as_str() {
            Some("predicate" | "noul") => "binary",
            Some("choice") => "choice",
            Some("score") => "score",
            Some("refusal") => "refusal",
            _ => return Err(invalid(format, "answers.type", "unsupported answer type")),
        };
        if q.is_some_and(|q| kind != "refusal" && q.kind != kind) {
            return Err(invalid(
                format,
                "answers.type",
                "answer type differs from question",
            ));
        }
        let mut value = match kind {
            "binary" => a[if format == FormatId::OpenAiDecisions {
                "probability"
            } else {
                "noul"
            }]
            .clone(),
            "choice" => a["choice"].clone(),
            "score" => a["score"].clone(),
            _ => Value::Null,
        };
        if kind == "binary" {
            probability(&value, format)?;
        }
        if kind == "score" && value.as_f64().is_none() {
            return Err(invalid(format, "answers.score", "expected a numeric score"));
        }
        if kind == "choice" && key_value_from_question(q, &value).is_none() {
            return Err(invalid(
                format,
                "answers.choice",
                "unknown or invalid choice",
            ));
        }
        if kind == "choice" {
            value = key_value_from_question(q, &value).unwrap_or(value);
        }
        let mut probabilities = Vec::new();
        if kind == "choice" || kind == "score" {
            if format == FormatId::OpenAiDecisions {
                for p in a["probabilities"]
                    .as_array()
                    .ok_or_else(|| invalid(format, "answers.probabilities", "expected an array"))?
                {
                    probability(&p["probability"], format)?;
                    probabilities.push(p.clone());
                }
            } else {
                for (k, p) in a["probabilities"]
                    .as_object()
                    .ok_or_else(|| invalid(format, "answers.probabilities", "expected a map"))?
                {
                    probability(p, format)?;
                    let v = if kind == "score" {
                        json!(k.parse::<u64>().map_err(|_| invalid(
                            format,
                            "answers.probabilities",
                            "invalid score index"
                        ))?)
                    } else {
                        key_value_from_question(q, &json!(k)).ok_or_else(|| {
                            invalid(format, "answers.probabilities", "unknown choice")
                        })?
                    };
                    let mut item = json!({"value":v,"probability":p});
                    if kind == "score" {
                        let label = a["legend"]
                            .get(k)
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .or_else(|| {
                                q.and_then(|q| q.options.get(v.as_u64()? as usize))
                                    .map(|o| o.description.clone())
                            })
                            .ok_or_else(|| {
                                invalid(format, "answers.legend", "missing score label")
                            })?;
                        item["label"] = json!(label);
                    }
                    probabilities.push(item);
                }
            }
        }
        let confidence = a.get("confidence").cloned();
        if let Some(c) = &confidence {
            probability(c, format)?;
        }
        answers.push(DecisionAnswer {
            key: key.clone(),
            name: q.map(|q| q.name.clone()).unwrap_or_else(|| {
                a.get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or(Some(key))
            }),
            kind: kind.to_string(),
            value,
            probabilities,
            confidence,
        });
    }
    if !questions.is_empty() {
        answers.sort_by_key(|a| {
            questions
                .iter()
                .position(|q| q.key == a.key)
                .unwrap_or(usize::MAX)
        });
    }
    let usage=body.get("usage").map(|u| if format==FormatId::GeminiDecisions {json!({"input_tokens":u["total_input_tokens"],"output_tokens":u["total_output_tokens"],"total_tokens":u["total_tokens"]})} else if format==FormatId::SystemOneDecisions {json!({"input_tokens":u["input_tokens"],"output_tokens":u["output_tokens"],"input_tokens_details":{"cached_tokens":u["cached_input_tokens"]}})} else {u.clone()});
    Ok(CanonicalResponse {
        id: String::new(),
        model: body["model"].as_str().unwrap_or_default().to_string(),
        outputs: Vec::new(),
        content: Vec::new(),
        stop_reason: None,
        usage: openai_usage_to_canonical(usage.as_ref()),
        extensions: BTreeMap::from([(EXTENSION.to_string(), json!(answers))]),
    })
}

fn key_value_from_question(question: Option<&DecisionQuestion>, value: &Value) -> Option<Value> {
    let k = key(value)?;
    match question {
        Some(q) => q
            .options
            .iter()
            .find(|o| key(&o.value).as_deref() == Some(&k))
            .map(|o| o.value.clone()),
        None => Some(value.clone()),
    }
}

pub fn emit_response(
    format: FormatId,
    response: &CanonicalResponse,
    _ctx: &FormatContext,
) -> Result<Value, FormatError> {
    let answers: Vec<DecisionAnswer> = serde_json::from_value(
        response
            .extensions
            .get(EXTENSION)
            .cloned()
            .unwrap_or(Value::Null),
    )
    .map_err(|_| invalid(format, "answers", "missing canonical Decisions answers"))?;
    let mut array = Vec::new();
    let mut map = Map::new();
    for a in answers {
        if a.kind == "refusal" && format != FormatId::OpenAiDecisions {
            return Err(invalid(
                format,
                "answers",
                "provider refused a question; target dialect has no refusal equivalent",
            ));
        }
        if a.kind == "binary" && format == FormatId::OpenAiDecisions && a.confidence.is_some() {
            return Err(invalid(
                format,
                "answers.confidence",
                "predicate has no equivalent for binary confidence",
            ));
        }
        if a.kind == "binary"
            && format == FormatId::GeminiDecisions
            && !a.value.as_f64().is_some_and(|p| (0.01..=0.99).contains(&p))
        {
            return Err(invalid(
                format,
                "answers.noul",
                "Gemini requires probability between 0.01 and 0.99",
            ));
        }
        let mut v = json!({"type":if a.kind=="binary" {if format==FormatId::OpenAiDecisions {"predicate"} else {"noul"}} else {&a.kind}});
        match a.kind.as_str() {
            "binary" => {
                v[if format == FormatId::OpenAiDecisions {
                    "probability"
                } else {
                    "noul"
                }] = a.value;
            }
            "choice" => {
                v["choice"] = if format == FormatId::OpenAiDecisions {
                    a.value
                } else {
                    json!(key(&a.value).ok_or_else(|| invalid(
                        format,
                        "answers.choice",
                        "invalid choice value"
                    ))?)
                };
            }
            "score" => {
                v["score"] = a.value;
            }
            _ => {}
        }
        if let Some(c) = a.confidence {
            v["confidence"] = c;
        }
        if a.kind == "choice" || a.kind == "score" {
            if format == FormatId::OpenAiDecisions {
                v["probabilities"] = json!(a.probabilities);
            } else {
                let mut probs = Map::new();
                let mut legend = Map::new();
                for p in a.probabilities {
                    let k = if a.kind == "score" {
                        p["value"].as_u64().map(|i| i.to_string())
                    } else {
                        key(&p["value"])
                    }
                    .ok_or_else(|| {
                        invalid(format, "answers.probabilities", "invalid option value")
                    })?;
                    if probs.insert(k.clone(), p["probability"].clone()).is_some() {
                        return Err(invalid(
                            format,
                            "answers.probabilities",
                            "choice values collide when stringified",
                        ));
                    }
                    if a.kind == "score" {
                        legend.insert(k, p["label"].clone());
                    }
                }
                v["probabilities"] = Value::Object(probs);
                if a.kind == "score" {
                    v["legend"] = Value::Object(legend);
                }
            }
        }
        if format == FormatId::OpenAiDecisions {
            v["name"] = json!(a.name);
            array.push(v);
        } else {
            map.insert(a.key, v);
        }
    }
    let mut body = json!({"model":response.model,"answers":if format==FormatId::OpenAiDecisions {Value::Array(array)} else {Value::Object(map)}});
    if let Some(u) = &response.usage {
        body["usage"] = if format == FormatId::OpenAiDecisions {
            canonical_usage_to_openai_responses_usage(u)
        } else if format == FormatId::GeminiDecisions {
            json!({"total_input_tokens":canonical_usage_total_input_tokens(u),"total_output_tokens":u.output_tokens,"total_tokens":u.total_tokens})
        } else {
            json!({"input_tokens":canonical_usage_total_input_tokens(u),"output_tokens":u.output_tokens,"cached_input_tokens":u.cache_read_tokens})
        };
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::registry::{convert_request, convert_response, parse_request as parse};

    const FORMATS: [&str; 3] = [
        "gemini:decisions",
        "openai:decisions",
        "systemone:decisions",
    ];

    fn request(format: &str) -> Value {
        if format == "openai:decisions" {
            json!({"model":"client-model","input":"The mug arrived cracked.","questions":[
                {"name":"damage","type":"predicate","instructions":"Is it damaged?"},
                {"name":"team","type":"choice","instructions":"Which team?","choices":[{"value":"billing","description":"Billing"},{"value":"shipping","description":"Shipping"}]},
                {"name":"severity","type":"score","instructions":"How severe?","levels":[{"label":"Cosmetic"},{"label":"Blocked"}]}
            ]})
        } else {
            json!({"model":"client-model",if format=="systemone:decisions" {"state"} else {"input"}:"The mug arrived cracked.","questions":{
                "damage":{"type":"noul","instructions":"Is it damaged?"},
                "team":{"type":"choice","instructions":"Which team?","criteria":{"billing":"Billing","shipping":"Shipping"}},
                "severity":{"type":"score","instructions":"How severe?","criteria":["Cosmetic","Blocked"]}
            }})
        }
    }

    fn response(format: &str) -> Value {
        if format == "openai:decisions" {
            json!({"model":"provider-model","answers":[
                {"name":"damage","type":"predicate","probability":0.98},
                {"name":"team","type":"choice","choice":"shipping","confidence":0.8,"probabilities":[{"value":"billing","probability":0.2},{"value":"shipping","probability":0.8}]},
                {"name":"severity","type":"score","score":0.8,"confidence":0.7,"probabilities":[{"value":0,"label":"Cosmetic","probability":0.2},{"value":1,"label":"Blocked","probability":0.8}]}
            ],"usage":{"input_tokens":100,"output_tokens":0,"total_tokens":100,"input_tokens_details":{"cached_tokens":20,"cache_write_tokens":0},"output_tokens_details":{"reasoning_tokens":0}}})
        } else {
            json!({"model":"provider-model","answers":{
                "damage":{"type":"noul","noul":0.98},
                "team":{"type":"choice","choice":"shipping","confidence":0.8,"probabilities":{"billing":0.2,"shipping":0.8}},
                "severity":{"type":"score","score":0.8,"confidence":0.7,"probabilities":{"0":0.2,"1":0.8},"legend":{"0":"Cosmetic","1":"Blocked"}}
            },"usage":if format=="gemini:decisions" {json!({"total_input_tokens":100,"total_output_tokens":0,"total_tokens":100})} else {json!({"input_tokens":100,"cached_input_tokens":20,"output_tokens":0})}})
        }
    }

    #[test]
    fn decisions_all_six_pairs_convert_binary_choice_score_and_usage() {
        for source in FORMATS {
            for target in FORMATS {
                if source == target {
                    continue;
                }
                let ctx = FormatContext::default().with_mapped_model("provider-model");
                let converted = convert_request(source, target, &request(source), &ctx).unwrap();
                let mut expected = request(target);
                expected["model"] = json!("provider-model");
                assert_eq!(converted, expected, "{source} -> {target}");
                let original = parse(target, &request(target), &FormatContext::default()).unwrap();
                let ctx =
                    ctx.with_report_context(json!({REQUEST_METADATA:request_metadata(&original)}));
                let converted = convert_response(source, target, &response(source), &ctx).unwrap();
                let answers = &converted["answers"];
                if target == "openai:decisions" {
                    assert_eq!(answers[0]["name"], "damage");
                    assert_eq!(answers[0]["probability"], 0.98);
                    assert_eq!(answers[2]["score"], 0.8);
                    assert_eq!(converted["usage"]["input_tokens"], 100);
                } else {
                    assert_eq!(answers["damage"]["noul"], 0.98);
                    assert_eq!(answers["team"]["choice"], "shipping");
                    assert_eq!(answers["severity"]["legend"]["1"], "Blocked");
                    assert_eq!(
                        converted["usage"][if target == "gemini:decisions" {
                            "total_input_tokens"
                        } else {
                            "input_tokens"
                        }],
                        100
                    );
                }
                assert_eq!(
                    converted["usage"][if target == "gemini:decisions" {
                        "total_output_tokens"
                    } else {
                        "output_tokens"
                    }],
                    0
                );
            }
        }
    }

    #[test]
    fn decisions_unnamed_order_and_boolean_values_round_trip() {
        let original = json!({"model":"client","input":"text","questions":[
            {"type":"choice","instructions":"Choose","choices":[{"value":true},{"value":false}]},
            {"name":"_question_0","type":"predicate","instructions":"Yes?"}
        ]});
        let parsed = parse("openai:decisions", &original, &FormatContext::default()).unwrap();
        for target in ["gemini:decisions", "systemone:decisions"] {
            let converted = convert_request(
                "openai:decisions",
                target,
                &original,
                &FormatContext::default(),
            )
            .unwrap();
            assert_eq!(
                converted["questions"]["_question_0_"]["criteria"],
                json!({"true":"","false":""})
            );
            let upstream = json!({"answers":{
                "_question_0":{"type":"noul","noul":0.8},
                "_question_0_":{"type":"choice","choice":"true","confidence":0.8,"probabilities":{"true":0.8,"false":0.2}}
            }});
            let ctx = FormatContext::default()
                .with_report_context(json!({REQUEST_METADATA:request_metadata(&parsed)}));
            let result = convert_response(target, "openai:decisions", &upstream, &ctx).unwrap();
            assert!(result["answers"][0]["name"].is_null());
            assert_eq!(result["answers"][0]["choice"], true);
            assert_eq!(result["answers"][1]["name"], "_question_0");
            assert!(result["answers"][0]["probabilities"]
                .as_array()
                .unwrap()
                .iter()
                .all(|p| p["value"].is_boolean()));
        }
    }

    #[test]
    fn decisions_reject_ambiguous_lossy_empty_and_stream_requests() {
        for target in ["gemini:decisions", "systemone:decisions"] {
            let mut body = request("openai:decisions");
            body["questions"][1]["name"] = json!("damage");
            assert!(
                convert_request("openai:decisions", target, &body, &FormatContext::default())
                    .unwrap_err()
                    .to_string()
                    .contains("duplicate")
            );
            let mut body = request("openai:decisions");
            body["questions"][1]["choices"] = json!([{"value":true},{"value":"true"}]);
            assert!(
                convert_request("openai:decisions", target, &body, &FormatContext::default())
                    .unwrap_err()
                    .to_string()
                    .contains("collide")
            );
        }
        for source in FORMATS {
            for target in FORMATS {
                if source == target {
                    continue;
                }
                for body in [
                    json!({}),
                    json!({"model":"m","input":"","questions":[]}),
                    json!({"model":"m","state":"s","questions":{}}),
                ] {
                    assert!(
                        convert_request(source, target, &body, &FormatContext::default()).is_err()
                    );
                }
                let mut body = request(source);
                body["stream"] = json!(true);
                assert!(convert_request(source, target, &body, &FormatContext::default()).is_err());
                assert!(convert_request(
                    source,
                    target,
                    &request(source),
                    &FormatContext::default().with_upstream_stream(true)
                )
                .is_err());
                assert!(convert_response(
                    source,
                    target,
                    &json!({"answers":null}),
                    &FormatContext::default()
                )
                .is_err());
                assert!(convert_response(
                    source,
                    target,
                    &json!({"answers":if source=="openai:decisions" {json!([])} else {json!({})}}),
                    &FormatContext::default()
                )
                .is_err());
            }
        }
    }

    #[test]
    fn decisions_refusal_confidence_and_modalities_fail_closed() {
        for target in ["gemini:decisions", "systemone:decisions"] {
            assert!(convert_response(
                "openai:decisions",
                target,
                &json!({"answers":[{"name":"q","type":"refusal"}]}),
                &FormatContext::default()
            )
            .unwrap_err()
            .to_string()
            .contains("refused"));
        }
        for source in ["gemini:decisions", "systemone:decisions"] {
            let mut body = response(source);
            body["answers"]["damage"]["confidence"] = json!(0.9);
            assert!(
                convert_response(source, "openai:decisions", &body, &FormatContext::default())
                    .unwrap_err()
                    .to_string()
                    .contains("confidence")
            );
        }
        let mut body = request("gemini:decisions");
        body["input"] = json!({"type":"image","uri":"https://example.com/image.png"});
        assert!(convert_request(
            "gemini:decisions",
            "systemone:decisions",
            &body,
            &FormatContext::default()
        )
        .unwrap_err()
        .to_string()
        .contains("modality"));
        let mut body = request("systemone:decisions");
        body["state"] = json!({"customer":{"tier":"Gold"}});
        let result = convert_request(
            "systemone:decisions",
            "gemini:decisions",
            &body,
            &FormatContext::default(),
        )
        .unwrap();
        assert_eq!(result["input"], json!(body["state"].to_string()));
        let result = convert_request(
            "systemone:decisions",
            "openai:decisions",
            &body,
            &FormatContext::default(),
        )
        .unwrap();
        assert_eq!(result["input"], body["state"]);
    }
}
