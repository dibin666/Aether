#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AntigravityCompatibilityProfile {
    pub name: &'static str,
    pub cli_version: &'static str,
    pub http_user_agent: &'static str,
    pub envelope_user_agent: &'static str,
    pub default_request_type: &'static str,
}

pub const ANTIGRAVITY_CLI_VERSION: &str = "1.1.9";
pub const ANTIGRAVITY_REQUEST_USER_AGENT: &str = "antigravity/cli/1.1.9 windows/amd64";
pub const ANTIGRAVITY_ENVELOPE_USER_AGENT: &str = "antigravity";
pub const ANTIGRAVITY_GOOGLE_ONE_AI_CREDIT_TYPE: &str = "GOOGLE_ONE_AI";

pub const ANTIGRAVITY_CLI_COMPATIBILITY_PROFILE: AntigravityCompatibilityProfile =
    AntigravityCompatibilityProfile {
        name: "cli_1_1_9",
        cli_version: ANTIGRAVITY_CLI_VERSION,
        http_user_agent: ANTIGRAVITY_REQUEST_USER_AGENT,
        envelope_user_agent: ANTIGRAVITY_ENVELOPE_USER_AGENT,
        default_request_type: "agent",
    };

pub fn current_antigravity_compatibility_profile() -> &'static AntigravityCompatibilityProfile {
    &ANTIGRAVITY_CLI_COMPATIBILITY_PROFILE
}

#[cfg(test)]
mod tests {
    use super::{
        current_antigravity_compatibility_profile, ANTIGRAVITY_ENVELOPE_USER_AGENT,
        ANTIGRAVITY_REQUEST_USER_AGENT,
    };

    #[test]
    fn current_profile_separates_http_and_envelope_identity() {
        let profile = current_antigravity_compatibility_profile();

        assert_eq!(profile.name, "cli_1_1_9");
        assert_eq!(profile.http_user_agent, ANTIGRAVITY_REQUEST_USER_AGENT);
        assert_eq!(profile.envelope_user_agent, ANTIGRAVITY_ENVELOPE_USER_AGENT);
        assert_ne!(profile.http_user_agent, profile.envelope_user_agent);
    }
}
