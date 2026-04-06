use std::fmt;

pub use self::sse::{encode_done_sse, encode_json_sse, map_claude_stop_reason};
pub use self::standard::stream_core::CanonicalStreamEvent;
pub use self::standard::stream_core::CanonicalStreamFrame;
pub use self::stream_rewrite::{resolve_finalize_stream_rewrite_mode, FinalizeStreamRewriteMode};

pub mod common;
pub mod sse;
pub mod standard;
pub mod stream_rewrite;
pub mod sync_products;

#[derive(Debug)]
pub struct PipelineFinalizeError(pub String);

impl PipelineFinalizeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for PipelineFinalizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pipeline finalize error: {}", self.0)
    }
}

impl std::error::Error for PipelineFinalizeError {}

impl From<serde_json::Error> for PipelineFinalizeError {
    fn from(source: serde_json::Error) -> Self {
        Self(source.to_string())
    }
}

impl From<base64::DecodeError> for PipelineFinalizeError {
    fn from(source: base64::DecodeError) -> Self {
        Self(source.to_string())
    }
}
