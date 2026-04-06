use crate::contracts::{
    OPENAI_CLI_STREAM_PLAN_KIND, OPENAI_CLI_SYNC_PLAN_KIND, OPENAI_COMPACT_STREAM_PLAN_KIND,
    OPENAI_COMPACT_SYNC_PLAN_KIND,
};

#[derive(Debug, Clone, Copy)]
pub struct LocalOpenAiCliSpec {
    pub api_format: &'static str,
    pub decision_kind: &'static str,
    pub report_kind: &'static str,
    pub compact: bool,
    pub require_streaming: bool,
}

pub fn resolve_sync_spec(plan_kind: &str) -> Option<LocalOpenAiCliSpec> {
    match plan_kind {
        OPENAI_CLI_SYNC_PLAN_KIND => Some(LocalOpenAiCliSpec {
            api_format: "openai:cli",
            decision_kind: OPENAI_CLI_SYNC_PLAN_KIND,
            report_kind: "openai_cli_sync_success",
            compact: false,
            require_streaming: false,
        }),
        OPENAI_COMPACT_SYNC_PLAN_KIND => Some(LocalOpenAiCliSpec {
            api_format: "openai:compact",
            decision_kind: OPENAI_COMPACT_SYNC_PLAN_KIND,
            report_kind: "openai_cli_sync_success",
            compact: true,
            require_streaming: false,
        }),
        _ => None,
    }
}

pub fn resolve_stream_spec(plan_kind: &str) -> Option<LocalOpenAiCliSpec> {
    match plan_kind {
        OPENAI_CLI_STREAM_PLAN_KIND => Some(LocalOpenAiCliSpec {
            api_format: "openai:cli",
            decision_kind: OPENAI_CLI_STREAM_PLAN_KIND,
            report_kind: "openai_cli_stream_success",
            compact: false,
            require_streaming: true,
        }),
        OPENAI_COMPACT_STREAM_PLAN_KIND => Some(LocalOpenAiCliSpec {
            api_format: "openai:compact",
            decision_kind: OPENAI_COMPACT_STREAM_PLAN_KIND,
            report_kind: "openai_cli_stream_success",
            compact: true,
            require_streaming: true,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_stream_spec, resolve_sync_spec};

    #[test]
    fn resolves_openai_cli_sync_spec() {
        let spec = resolve_sync_spec("openai_cli_sync").expect("spec");
        assert_eq!(spec.api_format, "openai:cli");
        assert_eq!(spec.report_kind, "openai_cli_sync_success");
        assert!(!spec.compact);
        assert!(!spec.require_streaming);
    }

    #[test]
    fn resolves_openai_compact_stream_spec() {
        let spec = resolve_stream_spec("openai_compact_stream").expect("spec");
        assert_eq!(spec.api_format, "openai:compact");
        assert_eq!(spec.report_kind, "openai_cli_stream_success");
        assert!(spec.compact);
        assert!(spec.require_streaming);
    }
}
