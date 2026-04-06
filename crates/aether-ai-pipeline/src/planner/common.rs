use base64::Engine as _;

pub fn parse_direct_request_body(
    is_json_request: bool,
    body_bytes: &[u8],
) -> Option<(serde_json::Value, Option<String>)> {
    if is_json_request {
        if body_bytes.is_empty() {
            Some((serde_json::json!({}), None))
        } else {
            serde_json::from_slice::<serde_json::Value>(body_bytes)
                .ok()
                .map(|value| (value, None))
        }
    } else {
        Some((
            serde_json::json!({}),
            (!body_bytes.is_empty())
                .then(|| base64::engine::general_purpose::STANDARD.encode(body_bytes)),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_direct_request_body;

    #[test]
    fn parses_empty_json_body_as_empty_object() {
        assert_eq!(
            parse_direct_request_body(true, b""),
            Some((serde_json::json!({}), None))
        );
    }

    #[test]
    fn rejects_invalid_json_body() {
        assert_eq!(parse_direct_request_body(true, b"{invalid"), None);
    }

    #[test]
    fn encodes_non_json_body_as_base64() {
        assert_eq!(
            parse_direct_request_body(false, b"hello"),
            Some((serde_json::json!({}), Some("aGVsbG8=".to_string())))
        );
    }
}
