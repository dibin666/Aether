use std::fmt;

use crate::formats::shared::multipart::{
    multipart_text_field, parse_multipart_fields, replace_multipart_text_field,
};

pub const TRANSCRIPTION_CONTENT_TYPE_DETAIL: &str =
    "Transcription request content-type must be multipart/form-data";
pub const TRANSCRIPTION_INVALID_MULTIPART_DETAIL: &str =
    "Transcription request multipart body is invalid";
pub const TRANSCRIPTION_MODEL_REQUIRED_DETAIL: &str = "Transcription request model is required";
pub const TRANSCRIPTION_FILE_REQUIRED_DETAIL: &str = "Transcription request file is required";
pub const TRANSCRIPTION_STREAM_INVALID_DETAIL: &str =
    "Transcription request stream must be true or false";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiTranscriptionRequestMetadata {
    pub requested_model: String,
    pub stream: bool,
    pub response_format: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiTranscriptionRequestError {
    ContentType,
    InvalidMultipart,
    ModelRequired,
    FileRequired,
    InvalidStream,
}

impl OpenAiTranscriptionRequestError {
    pub fn detail(self) -> &'static str {
        match self {
            Self::ContentType => TRANSCRIPTION_CONTENT_TYPE_DETAIL,
            Self::InvalidMultipart => TRANSCRIPTION_INVALID_MULTIPART_DETAIL,
            Self::ModelRequired => TRANSCRIPTION_MODEL_REQUIRED_DETAIL,
            Self::FileRequired => TRANSCRIPTION_FILE_REQUIRED_DETAIL,
            Self::InvalidStream => TRANSCRIPTION_STREAM_INVALID_DETAIL,
        }
    }
}

impl fmt::Display for OpenAiTranscriptionRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.detail())
    }
}

impl std::error::Error for OpenAiTranscriptionRequestError {}

pub fn parse_openai_transcription_request(
    content_type: Option<&str>,
    body: &[u8],
) -> Result<OpenAiTranscriptionRequestMetadata, OpenAiTranscriptionRequestError> {
    let content_type = content_type.ok_or(OpenAiTranscriptionRequestError::ContentType)?;
    if !content_type
        .split(';')
        .next()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("multipart/form-data"))
    {
        return Err(OpenAiTranscriptionRequestError::ContentType);
    }

    let fields = parse_multipart_fields(content_type, body)
        .map_err(|_| OpenAiTranscriptionRequestError::InvalidMultipart)?;

    let mut model_fields = fields.iter().filter(|field| field.name == "model");
    let model_field = model_fields
        .next()
        .ok_or(OpenAiTranscriptionRequestError::ModelRequired)?;
    if model_fields.next().is_some() {
        return Err(OpenAiTranscriptionRequestError::InvalidMultipart);
    }
    if model_field.filename.is_some() {
        return Err(OpenAiTranscriptionRequestError::ModelRequired);
    }
    let requested_model = std::str::from_utf8(model_field.data)
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(OpenAiTranscriptionRequestError::ModelRequired)?
        .to_string();

    if !fields
        .iter()
        .any(|field| field.name == "file" && field.filename.is_some() && !field.data.is_empty())
    {
        return Err(OpenAiTranscriptionRequestError::FileRequired);
    }

    let stream = parse_optional_stream(&fields)?;
    let response_format = multipart_text_field(&fields, "response_format")
        .map_err(|_| OpenAiTranscriptionRequestError::InvalidMultipart)?
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    Ok(OpenAiTranscriptionRequestMetadata {
        requested_model,
        stream,
        response_format,
    })
}

pub fn rewrite_openai_transcription_model(
    content_type: &str,
    body: &[u8],
    mapped_model: &str,
) -> Result<Vec<u8>, OpenAiTranscriptionRequestError> {
    parse_openai_transcription_request(Some(content_type), body)?;
    replace_multipart_text_field(content_type, body, "model", mapped_model)
        .map_err(|_| OpenAiTranscriptionRequestError::InvalidMultipart)
}

fn parse_optional_stream(
    fields: &[crate::formats::shared::multipart::MultipartField<'_>],
) -> Result<bool, OpenAiTranscriptionRequestError> {
    let mut stream_fields = fields.iter().filter(|field| field.name == "stream");
    let Some(field) = stream_fields.next() else {
        return Ok(false);
    };
    if stream_fields.next().is_some() || field.filename.is_some() {
        return Err(OpenAiTranscriptionRequestError::InvalidStream);
    }
    let value = std::str::from_utf8(field.data)
        .map_err(|_| OpenAiTranscriptionRequestError::InvalidStream)?
        .trim();
    if value.eq_ignore_ascii_case("true") {
        Ok(true)
    } else if value.eq_ignore_ascii_case("false") {
        Ok(false)
    } else {
        Err(OpenAiTranscriptionRequestError::InvalidStream)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_openai_transcription_request, rewrite_openai_transcription_model,
        OpenAiTranscriptionRequestError,
    };
    use crate::formats::shared::multipart::{multipart_text_field, parse_multipart_fields};

    const BOUNDARY: &str = "transcription-boundary";
    const CONTENT_TYPE: &str = "multipart/form-data; boundary=transcription-boundary";
    const WAV_BYTES: &[u8] = b"RIFF\0\x01--transcription-boundary\xffWAVE\r\nbytes";

    fn push_text(body: &mut Vec<u8>, name: &str, value: &str) {
        body.extend_from_slice(format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
    }

    fn valid_body(stream: Option<&str>) -> Vec<u8> {
        let mut body = Vec::new();
        push_text(&mut body, "model", "transcribe-client");
        push_text(&mut body, "language", "en");
        push_text(&mut body, "prompt", "meeting notes");
        push_text(&mut body, "response_format", "verbose_json");
        push_text(&mut body, "temperature", "0.2");
        push_text(&mut body, "timestamp_granularities[]", "word");
        push_text(&mut body, "include[]", "logprobs");
        push_text(&mut body, "chunking_strategy", "auto");
        push_text(&mut body, "known_speaker_names[]", "Alice");
        push_text(&mut body, "future_field", "future-value");
        if let Some(stream) = stream {
            push_text(&mut body, "stream", stream);
        }
        body.extend_from_slice(format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"known_speaker_references[]\"; filename=\"alice.wav\"\r\nContent-Type: audio/wav\r\n\r\nreference\r\n").as_bytes());
        body.extend_from_slice(format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"meeting.wav\"\r\nContent-Type: audio/wav\r\n\r\n").as_bytes());
        body.extend_from_slice(WAV_BYTES);
        body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
        body
    }

    #[test]
    fn openai_transcription_parses_all_fields_without_restricting_unknown_fields() {
        let body = valid_body(Some("TrUe"));
        let metadata = parse_openai_transcription_request(Some(CONTENT_TYPE), &body)
            .expect("request should parse");

        assert_eq!(metadata.requested_model, "transcribe-client");
        assert!(metadata.stream);
        assert_eq!(metadata.response_format.as_deref(), Some("verbose_json"));
    }

    #[test]
    fn openai_transcription_rewrites_only_model_and_preserves_binary_bytes() {
        let body = valid_body(Some("false"));
        let rewritten =
            rewrite_openai_transcription_model(CONTENT_TYPE, &body, "gpt-4o-transcribe-upstream")
                .expect("model should rewrite");
        let fields = parse_multipart_fields(CONTENT_TYPE, &rewritten).expect("body should parse");

        assert_eq!(
            multipart_text_field(&fields, "model").unwrap(),
            Some("gpt-4o-transcribe-upstream")
        );
        assert_eq!(
            fields
                .iter()
                .find(|field| field.name == "file")
                .unwrap()
                .data,
            WAV_BYTES
        );
        assert_eq!(
            multipart_text_field(&fields, "future_field").unwrap(),
            Some("future-value")
        );
    }

    #[test]
    fn openai_transcription_reports_fixed_validation_categories() {
        assert_eq!(
            parse_openai_transcription_request(None, b"").unwrap_err(),
            OpenAiTranscriptionRequestError::ContentType
        );
        assert_eq!(
            parse_openai_transcription_request(
                Some("multipart/form-data; boundary=bad"),
                b"not-multipart"
            )
            .unwrap_err(),
            OpenAiTranscriptionRequestError::InvalidMultipart
        );

        let mut missing_model = valid_body(None);
        let model = b"name=\"model\"";
        let offset = missing_model
            .windows(model.len())
            .position(|window| window == model)
            .unwrap();
        missing_model[offset + 6..offset + 11].copy_from_slice(b"other");
        assert_eq!(
            parse_openai_transcription_request(Some(CONTENT_TYPE), &missing_model).unwrap_err(),
            OpenAiTranscriptionRequestError::ModelRequired
        );

        let mut missing_file = valid_body(None);
        let file = b"name=\"file\"";
        let offset = missing_file
            .windows(file.len())
            .position(|window| window == file)
            .unwrap();
        missing_file[offset + 6..offset + 10].copy_from_slice(b"data");
        assert_eq!(
            parse_openai_transcription_request(Some(CONTENT_TYPE), &missing_file).unwrap_err(),
            OpenAiTranscriptionRequestError::FileRequired
        );

        let invalid_stream = valid_body(Some("yes"));
        assert_eq!(
            parse_openai_transcription_request(Some(CONTENT_TYPE), &invalid_stream).unwrap_err(),
            OpenAiTranscriptionRequestError::InvalidStream
        );
    }
}
