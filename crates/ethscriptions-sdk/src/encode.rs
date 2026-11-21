// SPDX-License-Identifier: MPL-2.0

use std::io::Write;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use flate2::Compression;
use flate2::write::GzEncoder;

use crate::digest::compute_digest;
use crate::error::ApiError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputSource {
    Literal,
    File,
    Stdin,
}

#[derive(Debug, Clone, Default)]
pub struct DataUriOptions {
    pub skip_content_type: bool,
    pub esip6: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrepareInscribeOptions {
    pub source: InputSource,
    pub optimize: bool,
    pub gzip: bool,
    pub esip6: bool,
    pub skip_content_type: bool,
    pub data_uri_limit: usize,
}

impl Default for PrepareInscribeOptions {
    fn default() -> Self {
        Self {
            source: InputSource::Literal,
            optimize: false,
            gzip: false,
            esip6: false,
            skip_content_type: false,
            data_uri_limit: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PreparedInscribe {
    pub content_type: String,
    pub data_uri: String,
    pub data_uri_truncated: String,
    pub gzipped_size: Option<usize>,
    pub sha: String,
    pub size: usize,
}

pub fn prepare_payload(
    bytes: &[u8],
    options: &PrepareInscribeOptions,
) -> Result<PreparedInscribe, ApiError> {
    let mut content_type = detect_content_type(bytes);
    let mut transformed = bytes.to_vec();
    let original_size = transformed.len();

    if options.optimize
        && let Some(optimized) = optimize_image(&transformed, &content_type)?
    {
        transformed = optimized;
        content_type = "image/webp".to_string();
    }

    let gzipped_size = if options.gzip && options.source != InputSource::Literal {
        transformed = gzip_bytes(&transformed)?;
        Some(transformed.len())
    } else {
        None
    };

    let data_uri = build_data_uri_with_type(
        &transformed,
        &content_type,
        &DataUriOptions {
            skip_content_type: options.skip_content_type,
            esip6: options.esip6,
        },
    );
    let sha = compute_digest(data_uri.as_bytes());

    Ok(PreparedInscribe {
        content_type,
        data_uri_truncated: truncate_data_uri(&data_uri, options.data_uri_limit),
        data_uri,
        gzipped_size,
        sha,
        size: original_size,
    })
}

fn truncate_data_uri(data_uri: &str, limit: usize) -> String {
    let mut chars = data_uri.chars();
    let truncated: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

pub fn detect_content_type(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        if serde_json::from_str::<serde_json::Value>(text).is_ok() {
            return "application/json".to_string();
        }

        let trimmed = text.trim();
        if trimmed.starts_with("<svg") || trimmed.starts_with("<?xml") {
            return "image/svg+xml".to_string();
        }

        return "text/plain".to_string();
    }

    if let Some(kind) = infer::get(bytes) {
        return kind.mime_type().to_string();
    }

    "application/octet-stream".to_string()
}

pub fn build_data_uri(bytes: &[u8], options: &DataUriOptions) -> String {
    build_data_uri_with_type(bytes, &detect_content_type(bytes), options)
}

pub fn build_data_uri_with_type(
    bytes: &[u8],
    content_type: &str,
    options: &DataUriOptions,
) -> String {
    let mut prefix = String::from("data:");
    if !options.skip_content_type {
        prefix.push_str(content_type);
    }
    if options.esip6 {
        prefix.push_str(";rule=esip6");
    }

    if should_use_plain_data_uri(content_type) {
        let payload = percent_encode_bytes(bytes);
        format!("{prefix},{payload}")
    } else {
        let payload = BASE64.encode(bytes);
        format!("{prefix};base64,{payload}")
    }
}

pub fn should_use_plain_data_uri(content_type: &str) -> bool {
    content_type.starts_with("text/")
        || content_type == "application/json"
        || content_type == "image/svg+xml"
}

pub fn percent_encode_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &byte in bytes {
        let ch = byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '_' | '~') {
            out.push(ch);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

/// Parse a `data:` URI into raw bytes and content type.
///
/// Supports `data:[<mediatype>][;base64],<data>` format.
pub fn parse_data_uri(uri: &str) -> Result<(Vec<u8>, String), ApiError> {
    let rest = uri
        .strip_prefix("data:")
        .ok_or_else(|| ApiError::internal("Invalid data URI"))?;

    let (meta, data) = rest
        .split_once(',')
        .ok_or_else(|| ApiError::internal("Invalid data URI: missing comma"))?;

    let is_base64 = meta.ends_with(";base64");
    let content_type = if is_base64 {
        meta.strip_suffix(";base64").unwrap_or(meta)
    } else {
        meta
    };
    let content_type = if content_type.is_empty() {
        "text/plain;charset=US-ASCII"
    } else {
        content_type
    };

    let bytes = if is_base64 {
        BASE64
            .decode(data)
            .map_err(|e| ApiError::internal(format!("Failed to decode base64 data URI: {e}")))?
    } else {
        percent_decode(data)?
    };

    Ok((bytes, content_type.to_string()))
}

fn percent_decode(input: &str) -> Result<Vec<u8>, ApiError> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(ApiError::internal(
                    "Invalid percent-encoding in data URI payload",
                ));
            }

            let hex = &input[index + 1..index + 3];
            let value = u8::from_str_radix(hex, 16)
                .map_err(|_| ApiError::internal("Invalid percent-encoding in data URI payload"))?;
            decoded.push(value);
            index += 3;
            continue;
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    Ok(decoded)
}

pub fn gzip_bytes(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(bytes)
        .map_err(|e| ApiError::internal(format!("Failed to gzip input: {e}")))?;
    encoder
        .finish()
        .map_err(|e| ApiError::internal(format!("Failed to finish gzip stream: {e}")))
}

pub fn ungzip_bytes(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
    let mut decoder = flate2::read::GzDecoder::new(bytes);
    let mut output = Vec::new();
    std::io::Read::read_to_end(&mut decoder, &mut output)
        .map_err(|e| ApiError::internal(format!("Failed to ungzip content: {e}")))?;
    Ok(output)
}

pub fn optimize_image(bytes: &[u8], content_type: &str) -> Result<Option<Vec<u8>>, ApiError> {
    match content_type {
        "image/png" => optimize_to_webp(bytes, 100.0).map(Some),
        "image/jpeg" => optimize_to_webp(bytes, 95.0).map(Some),
        _ => Ok(None),
    }
}

fn optimize_to_webp(bytes: &[u8], quality: f32) -> Result<Vec<u8>, ApiError> {
    let image = image::load_from_memory(bytes)
        .map_err(|e| ApiError::bad_request(format!("Failed to decode image: {e}")))?;
    let encoder = webp::Encoder::from_image(&image)
        .map_err(|e| ApiError::internal(format!("Failed to prepare WebP image: {e}")))?;
    let webp = encoder.encode(quality);
    Ok(webp.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_data_uri_plain_text() {
        let (bytes, ct) = parse_data_uri("data:,hello").unwrap();
        assert_eq!(bytes, b"hello");
        // empty mediatype defaults to text/plain;charset=US-ASCII
        assert_eq!(ct, "text/plain;charset=US-ASCII");
    }

    #[test]
    fn test_parse_data_uri_with_mediatype() {
        let (bytes, ct) = parse_data_uri("data:text/plain,hello world").unwrap();
        assert_eq!(bytes, b"hello world");
        assert_eq!(ct, "text/plain");
    }

    #[test]
    fn test_parse_data_uri_base64() {
        // "Hello" in base64 is "SGVsbG8="
        let (bytes, ct) = parse_data_uri("data:text/plain;base64,SGVsbG8=").unwrap();
        assert_eq!(bytes, b"Hello");
        assert_eq!(ct, "text/plain");
    }

    #[test]
    fn test_parse_data_uri_base64_no_mediatype() {
        // ;base64 with empty mediatype
        let (bytes, ct) = parse_data_uri("data:;base64,SGVsbG8=").unwrap();
        assert_eq!(bytes, b"Hello");
        assert_eq!(ct, "text/plain;charset=US-ASCII");
    }

    #[test]
    fn test_parse_data_uri_image_base64() {
        // Valid base64 for a few bytes (PNG magic bytes: 0x89 0x50 0x4E 0x47)
        let (bytes, ct) = parse_data_uri("data:image/png;base64,iVBORw==").unwrap();
        assert_eq!(ct, "image/png");
        assert_eq!(bytes.is_empty(), false);
        assert_eq!(bytes[0], 0x89); // PNG magic byte
    }

    #[test]
    fn test_parse_data_uri_json() {
        let (bytes, ct) = parse_data_uri("data:application/json,{\"key\":\"value\"}").unwrap();
        assert_eq!(ct, "application/json");
        assert_eq!(std::str::from_utf8(&bytes).unwrap(), "{\"key\":\"value\"}");
    }

    #[test]
    fn test_parse_data_uri_empty_data() {
        let (bytes, ct) = parse_data_uri("data:text/plain,").unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(ct, "text/plain");
    }

    #[test]
    fn test_parse_data_uri_with_charset_param() {
        let (bytes, ct) = parse_data_uri("data:text/plain;charset=utf-8,hello").unwrap();
        assert_eq!(bytes, b"hello");
        assert_eq!(ct, "text/plain;charset=utf-8");
    }

    #[test]
    fn test_parse_data_uri_percent_decodes_payload() {
        let (bytes, ct) = parse_data_uri("data:text/plain,hello%20world%21").unwrap();
        assert_eq!(bytes, b"hello world!");
        assert_eq!(ct, "text/plain");
    }

    #[test]
    fn test_parse_data_uri_missing_comma() {
        let result = parse_data_uri("data:text/plain");
        assert_eq!(result.is_err(), true);
    }

    #[test]
    fn test_parse_data_uri_no_prefix() {
        let result = parse_data_uri("http://example.com");
        assert_eq!(result.is_err(), true);
    }

    #[test]
    fn test_parse_data_uri_invalid_base64() {
        let result = parse_data_uri("data:text/plain;base64,!!!invalid!!!");
        assert_eq!(result.is_err(), true);
    }
}
