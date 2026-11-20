// SPDX-License-Identifier: MPL-2.0

use std::io::Write;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use flate2::Compression;
use flate2::write::GzEncoder;

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
