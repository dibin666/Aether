use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultipartParseError {
    InvalidContentType,
    MissingBoundary,
    InvalidBody,
    InvalidUtf8Text,
    MissingField,
    DuplicateField,
    InvalidReplacement,
}

impl fmt::Display for MultipartParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidContentType => "content type is not multipart/form-data",
            Self::MissingBoundary => "multipart boundary is missing or invalid",
            Self::InvalidBody => "multipart body is invalid",
            Self::InvalidUtf8Text => "multipart text field is not valid UTF-8",
            Self::MissingField => "multipart field is missing",
            Self::DuplicateField => "multipart field is duplicated",
            Self::InvalidReplacement => "multipart replacement contains invalid framing bytes",
        })
    }
}

impl std::error::Error for MultipartParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultipartField<'a> {
    pub name: &'a str,
    pub filename: Option<&'a str>,
    pub content_type: Option<&'a str>,
    pub headers: &'a [u8],
    pub data: &'a [u8],
    data_start: usize,
    data_end: usize,
}

pub fn parse_multipart_fields<'a>(
    content_type: &str,
    body: &'a [u8],
) -> Result<Vec<MultipartField<'a>>, MultipartParseError> {
    let boundary = multipart_boundary(content_type)?;
    let marker = format!("--{boundary}").into_bytes();
    if !body.starts_with(&marker) {
        return Err(MultipartParseError::InvalidBody);
    }

    let mut cursor = marker.len();
    if body.get(cursor..cursor + 2) == Some(b"--") {
        validate_final_boundary_suffix(&body[cursor + 2..])?;
        return Ok(Vec::new());
    }
    if body.get(cursor..cursor + 2) != Some(b"\r\n") {
        return Err(MultipartParseError::InvalidBody);
    }
    cursor += 2;

    let mut fields = Vec::new();
    loop {
        let boundary_start = find_next_framed_boundary(body, cursor, &marker)
            .ok_or(MultipartParseError::InvalidBody)?;
        let raw_part = &body[cursor..boundary_start];
        fields.push(parse_multipart_part(raw_part, cursor)?);

        let marker_end = boundary_start + 2 + marker.len();
        if body.get(marker_end..marker_end + 2) == Some(b"--") {
            validate_final_boundary_suffix(&body[marker_end + 2..])?;
            break;
        }
        if body.get(marker_end..marker_end + 2) != Some(b"\r\n") {
            return Err(MultipartParseError::InvalidBody);
        }
        cursor = marker_end + 2;
    }

    Ok(fields)
}

pub fn multipart_text_field<'a>(
    fields: &'a [MultipartField<'a>],
    name: &str,
) -> Result<Option<&'a str>, MultipartParseError> {
    let Some(field) = fields.iter().find(|field| field.name == name) else {
        return Ok(None);
    };
    if field.filename.is_some() {
        return Err(MultipartParseError::InvalidBody);
    }
    std::str::from_utf8(field.data)
        .map(Some)
        .map_err(|_| MultipartParseError::InvalidUtf8Text)
}

pub fn count_non_empty_multipart_files(fields: &[MultipartField<'_>], name: &str) -> usize {
    fields
        .iter()
        .filter(|field| field.name == name && field.filename.is_some() && !field.data.is_empty())
        .count()
}

pub fn replace_multipart_text_field(
    content_type: &str,
    body: &[u8],
    name: &str,
    replacement: &str,
) -> Result<Vec<u8>, MultipartParseError> {
    if replacement
        .as_bytes()
        .iter()
        .any(|byte| matches!(byte, b'\r' | b'\n'))
    {
        return Err(MultipartParseError::InvalidReplacement);
    }
    let fields = parse_multipart_fields(content_type, body)?;
    let mut matches = fields
        .iter()
        .filter(|field| field.name == name && field.filename.is_none());
    let field = matches.next().ok_or(MultipartParseError::MissingField)?;
    if matches.next().is_some() {
        return Err(MultipartParseError::DuplicateField);
    }

    let replacement_bytes = replacement.as_bytes();
    let new_len = body.len() - field.data.len() + replacement_bytes.len();
    let mut rewritten = Vec::with_capacity(new_len);
    rewritten.extend_from_slice(&body[..field.data_start]);
    rewritten.extend_from_slice(replacement_bytes);
    rewritten.extend_from_slice(&body[field.data_end..]);
    Ok(rewritten)
}

fn multipart_boundary(content_type: &str) -> Result<&str, MultipartParseError> {
    let mut segments = content_type.split(';');
    let media_type = segments.next().unwrap_or_default().trim();
    if !media_type.eq_ignore_ascii_case("multipart/form-data") {
        return Err(MultipartParseError::InvalidContentType);
    }

    for segment in segments {
        let Some((key, value)) = segment.trim().split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("boundary") {
            continue;
        }
        let value = value.trim();
        let boundary = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            &value[1..value.len() - 1]
        } else {
            value
        };
        if boundary.is_empty()
            || boundary
                .as_bytes()
                .iter()
                .any(|byte| matches!(byte, b'\r' | b'\n'))
        {
            return Err(MultipartParseError::MissingBoundary);
        }
        return Ok(boundary);
    }

    Err(MultipartParseError::MissingBoundary)
}

fn find_next_framed_boundary(body: &[u8], start: usize, marker: &[u8]) -> Option<usize> {
    let mut cursor = start;
    while let Some(relative) = find_subslice(&body[cursor..], b"\r\n--") {
        let candidate = cursor + relative;
        let marker_start = candidate + 2;
        if body.get(marker_start..marker_start + marker.len()) == Some(marker) {
            let suffix = marker_start + marker.len();
            if matches!(body.get(suffix..suffix + 2), Some(b"\r\n") | Some(b"--")) {
                return Some(candidate);
            }
        }
        cursor = candidate + 2;
    }
    None
}

fn validate_final_boundary_suffix(suffix: &[u8]) -> Result<(), MultipartParseError> {
    if suffix.is_empty() || suffix == b"\r\n" {
        Ok(())
    } else {
        Err(MultipartParseError::InvalidBody)
    }
}

fn parse_multipart_part<'a>(
    raw_part: &'a [u8],
    body_offset: usize,
) -> Result<MultipartField<'a>, MultipartParseError> {
    let header_end =
        find_subslice(raw_part, b"\r\n\r\n").ok_or(MultipartParseError::InvalidBody)?;
    let headers = &raw_part[..header_end];
    let data_start_in_part = header_end + 4;
    let data = raw_part
        .get(data_start_in_part..)
        .ok_or(MultipartParseError::InvalidBody)?;

    let header_text = std::str::from_utf8(headers).map_err(|_| MultipartParseError::InvalidBody)?;
    let mut name = None;
    let mut filename = None;
    let mut content_type = None;
    let mut disposition_seen = false;

    for line in header_text.split("\r\n") {
        let Some((header_name, header_value)) = line.split_once(':') else {
            return Err(MultipartParseError::InvalidBody);
        };
        if header_name
            .trim()
            .eq_ignore_ascii_case("content-disposition")
        {
            if disposition_seen {
                return Err(MultipartParseError::InvalidBody);
            }
            disposition_seen = true;
            let mut parameters = header_value.trim().split(';');
            if !parameters
                .next()
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("form-data"))
            {
                return Err(MultipartParseError::InvalidBody);
            }
            for parameter in parameters {
                let Some((key, value)) = parameter.trim().split_once('=') else {
                    continue;
                };
                let value = trim_quoted_parameter(value.trim())?;
                if key.trim().eq_ignore_ascii_case("name") {
                    name = Some(value);
                } else if key.trim().eq_ignore_ascii_case("filename") {
                    filename = Some(value);
                }
            }
        } else if header_name.trim().eq_ignore_ascii_case("content-type") {
            let value = header_value.trim();
            if !value.is_empty() {
                content_type = Some(value);
            }
        }
    }

    let name = name
        .filter(|value| !value.is_empty())
        .ok_or(MultipartParseError::InvalidBody)?;
    let data_start = body_offset + data_start_in_part;
    Ok(MultipartField {
        name,
        filename,
        content_type,
        headers,
        data,
        data_start,
        data_end: data_start + data.len(),
    })
}

fn trim_quoted_parameter(value: &str) -> Result<&str, MultipartParseError> {
    if value.starts_with('"') {
        if value.len() < 2 || !value.ends_with('"') {
            return Err(MultipartParseError::InvalidBody);
        }
        Ok(&value[1..value.len() - 1])
    } else {
        Ok(value)
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::{
        count_non_empty_multipart_files, multipart_text_field, parse_multipart_fields,
        replace_multipart_text_field, MultipartParseError,
    };

    const CONTENT_TYPE: &str = "Multipart/Form-Data; BOUNDARY=audio-boundary";

    fn multipart_body() -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(
            b"--audio-boundary\r\nContent-Disposition: form-data; NAME=\"model\"\r\n\r\nclient-model\r\n",
        );
        body.extend_from_slice(
            b"--audio-boundary\r\nCONTENT-DISPOSITION: form-data; name=\"file\"; FILENAME=\"sample.wav\"\r\nContent-Type: audio/wav\r\n\r\n",
        );
        body.extend_from_slice(b"RIFF\0--audio-boundary\xff\r\nkept\r\n");
        body.extend_from_slice(b"--audio-boundary--\r\n");
        body
    }

    #[test]
    fn parses_binary_fields_only_at_framed_boundaries() {
        let body = multipart_body();
        let fields = parse_multipart_fields(CONTENT_TYPE, &body).expect("multipart should parse");

        assert_eq!(fields.len(), 2);
        assert_eq!(
            multipart_text_field(&fields, "model").unwrap(),
            Some("client-model")
        );
        assert_eq!(count_non_empty_multipart_files(&fields, "file"), 1);
        assert_eq!(fields[1].filename, Some("sample.wav"));
        assert_eq!(fields[1].content_type, Some("audio/wav"));
        assert_eq!(fields[1].data, b"RIFF\0--audio-boundary\xff\r\nkept");
    }

    #[test]
    fn replaces_only_the_target_text_data() {
        let body = multipart_body();
        let rewritten =
            replace_multipart_text_field(CONTENT_TYPE, &body, "model", "provider-model-longer")
                .expect("model should rewrite");
        let fields =
            parse_multipart_fields(CONTENT_TYPE, &rewritten).expect("rewrite should parse");

        assert_eq!(
            multipart_text_field(&fields, "model").unwrap(),
            Some("provider-model-longer")
        );
        assert_eq!(fields[1].data, b"RIFF\0--audio-boundary\xff\r\nkept");
        assert_eq!(fields[1].headers, b"CONTENT-DISPOSITION: form-data; name=\"file\"; FILENAME=\"sample.wav\"\r\nContent-Type: audio/wav");
    }

    #[test]
    fn rejects_unframed_or_unterminated_multipart() {
        assert_eq!(
            parse_multipart_fields("application/json", b"{}").unwrap_err(),
            MultipartParseError::InvalidContentType
        );
        assert_eq!(
            parse_multipart_fields("multipart/form-data", b"").unwrap_err(),
            MultipartParseError::MissingBoundary
        );
        assert_eq!(
            parse_multipart_fields(
                "multipart/form-data; boundary=x",
                b"--x\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\na"
            )
            .unwrap_err(),
            MultipartParseError::InvalidBody
        );
    }
}
