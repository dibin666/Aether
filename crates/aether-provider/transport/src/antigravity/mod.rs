mod auth;
mod normalize;
mod policy;
mod profile;
mod request;
mod url;

pub use auth::{
    build_antigravity_static_client_headers, build_antigravity_static_identity_headers,
    finalize_antigravity_request_headers, resolve_local_antigravity_request_auth,
    AntigravityRequestAuth, AntigravityRequestAuthSupport, AntigravityRequestAuthUnsupportedReason,
    ANTIGRAVITY_PROVIDER_TYPE, ANTIGRAVITY_REQUEST_USER_AGENT,
};
pub use policy::{
    classify_local_antigravity_request_support, is_antigravity_provider_transport,
    AntigravityRequestSideSpec, AntigravityRequestSideSupport,
    AntigravityRequestSideUnsupportedReason,
};
pub use profile::{
    current_antigravity_compatibility_profile, AntigravityCompatibilityProfile,
    ANTIGRAVITY_CLI_COMPATIBILITY_PROFILE, ANTIGRAVITY_CLI_VERSION,
    ANTIGRAVITY_ENVELOPE_USER_AGENT, ANTIGRAVITY_GOOGLE_ONE_AI_CREDIT_TYPE,
};
pub use request::{
    build_antigravity_safe_v1internal_request, classify_antigravity_safe_request_body,
    AntigravityEnvelopeRequestType, AntigravityRequestEnvelopeSupport,
    AntigravityRequestEnvelopeUnsupportedReason,
};
pub use url::{
    build_antigravity_v1internal_url, resolve_antigravity_api_base_url,
    AntigravityRequestUrlAction, ANTIGRAVITY_V1INTERNAL_PATH_TEMPLATE,
};
