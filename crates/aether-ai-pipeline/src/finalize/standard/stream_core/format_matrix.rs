use serde_json::Value;

use crate::finalize::standard::claude::stream::{ClaudeClientEmitter, ClaudeProviderState};
use crate::finalize::standard::gemini::stream::{GeminiClientEmitter, GeminiProviderState};
use crate::finalize::standard::openai::stream::{
    OpenAIChatClientEmitter, OpenAIChatProviderState, OpenAICliClientEmitter,
    OpenAICliProviderState,
};
use crate::finalize::standard::stream_core::common::CanonicalStreamFrame;
use crate::finalize::PipelineFinalizeError;

#[derive(Default)]
pub struct StreamingStandardFormatMatrix {
    provider: Option<ProviderStreamParser>,
    client: Option<ClientStreamEmitter>,
}

impl StreamingStandardFormatMatrix {
    pub fn transform_line(
        &mut self,
        report_context: &Value,
        line: Vec<u8>,
    ) -> Result<Vec<u8>, PipelineFinalizeError> {
        self.ensure_initialized(report_context);
        let Some(provider) = self.provider.as_mut() else {
            return Ok(Vec::new());
        };
        let frames = provider.push_line(report_context, line)?;
        self.emit_frames(frames)
    }

    pub fn finish(&mut self, report_context: &Value) -> Result<Vec<u8>, PipelineFinalizeError> {
        self.ensure_initialized(report_context);
        let Some(provider) = self.provider.as_mut() else {
            return Ok(Vec::new());
        };
        let frames = provider.finish(report_context)?;
        let mut out = self.emit_frames(frames)?;
        if let Some(client) = self.client.as_mut() {
            out.extend(client.finish()?);
        }
        Ok(out)
    }

    fn ensure_initialized(&mut self, report_context: &Value) {
        if self.provider.is_some() && self.client.is_some() {
            return;
        }

        let provider_api_format = report_context
            .get("provider_api_format")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let client_api_format = report_context
            .get("client_api_format")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();

        self.provider = ProviderStreamParser::for_api_format(provider_api_format.as_str());
        self.client = ClientStreamEmitter::for_api_format(client_api_format.as_str());
    }

    fn emit_frames(
        &mut self,
        frames: Vec<CanonicalStreamFrame>,
    ) -> Result<Vec<u8>, PipelineFinalizeError> {
        let Some(client) = self.client.as_mut() else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for frame in frames {
            out.extend(client.emit(frame)?);
        }
        Ok(out)
    }
}

enum ProviderStreamParser {
    OpenAIChat(OpenAIChatProviderState),
    OpenAICli(OpenAICliProviderState),
    Claude(ClaudeProviderState),
    Gemini(GeminiProviderState),
}

impl ProviderStreamParser {
    fn for_api_format(provider_api_format: &str) -> Option<Self> {
        Some(match provider_api_format {
            "openai:chat" => Self::OpenAIChat(OpenAIChatProviderState::default()),
            "openai:cli" | "openai:compact" => Self::OpenAICli(OpenAICliProviderState::default()),
            "claude:chat" | "claude:cli" => Self::Claude(ClaudeProviderState::default()),
            "gemini:chat" | "gemini:cli" => Self::Gemini(GeminiProviderState::default()),
            _ => return None,
        })
    }

    fn push_line(
        &mut self,
        report_context: &Value,
        line: Vec<u8>,
    ) -> Result<Vec<CanonicalStreamFrame>, PipelineFinalizeError> {
        match self {
            ProviderStreamParser::OpenAIChat(state) => state.push_line(report_context, line),
            ProviderStreamParser::OpenAICli(state) => state.push_line(report_context, line),
            ProviderStreamParser::Claude(state) => state.push_line(report_context, line),
            ProviderStreamParser::Gemini(state) => state.push_line(report_context, line),
        }
    }

    fn finish(
        &mut self,
        report_context: &Value,
    ) -> Result<Vec<CanonicalStreamFrame>, PipelineFinalizeError> {
        match self {
            ProviderStreamParser::OpenAIChat(state) => state.finish(report_context),
            ProviderStreamParser::OpenAICli(state) => state.finish(report_context),
            ProviderStreamParser::Claude(state) => state.finish(report_context),
            ProviderStreamParser::Gemini(state) => state.finish(report_context),
        }
    }
}

enum ClientStreamEmitter {
    OpenAIChat(OpenAIChatClientEmitter),
    OpenAICli(OpenAICliClientEmitter),
    Claude(ClaudeClientEmitter),
    Gemini(GeminiClientEmitter),
}

impl ClientStreamEmitter {
    fn for_api_format(client_api_format: &str) -> Option<Self> {
        Some(match client_api_format {
            "openai:chat" => Self::OpenAIChat(OpenAIChatClientEmitter::default()),
            "openai:cli" | "openai:compact" => Self::OpenAICli(OpenAICliClientEmitter::default()),
            "claude:chat" | "claude:cli" => Self::Claude(ClaudeClientEmitter::default()),
            "gemini:chat" | "gemini:cli" => Self::Gemini(GeminiClientEmitter::default()),
            _ => return None,
        })
    }

    fn emit(&mut self, frame: CanonicalStreamFrame) -> Result<Vec<u8>, PipelineFinalizeError> {
        match self {
            ClientStreamEmitter::OpenAIChat(state) => state.emit(frame),
            ClientStreamEmitter::OpenAICli(state) => state.emit(frame),
            ClientStreamEmitter::Claude(state) => state.emit(frame),
            ClientStreamEmitter::Gemini(state) => state.emit(frame),
        }
    }

    fn finish(&mut self) -> Result<Vec<u8>, PipelineFinalizeError> {
        match self {
            ClientStreamEmitter::OpenAIChat(state) => state.finish(),
            ClientStreamEmitter::OpenAICli(state) => state.finish(),
            ClientStreamEmitter::Claude(state) => state.finish(),
            ClientStreamEmitter::Gemini(state) => state.finish(),
        }
    }
}
