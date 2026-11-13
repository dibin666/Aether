pub(crate) fn normalized_signature(api_format: &str) -> Option<&'static str> {
    match crate::ai_serving::normalize_api_format_alias(api_format).as_str() {
        "systemone:decisions" => Some("systemone:decisions"),
        _ => None,
    }
}

pub(crate) fn local_path(api_format: &str) -> Option<&'static str> {
    match crate::ai_serving::normalize_api_format_alias(api_format).as_str() {
        "systemone:decisions" => Some("/v1/systemone"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{local_path, normalized_signature};

    #[test]
    fn maps_systemone_decisions_signature_and_path() {
        for alias in [
            "systemone:decisions",
            "systemone",
            "system_one",
            "typesafe:systemone",
            "/v1/systemone",
        ] {
            assert_eq!(normalized_signature(alias), Some("systemone:decisions"));
            assert_eq!(local_path(alias), Some("/v1/systemone"));
        }
    }
}
