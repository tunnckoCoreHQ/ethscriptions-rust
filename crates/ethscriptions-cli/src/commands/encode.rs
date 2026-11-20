// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use base64::Engine;
use clap::Args;
use ethscriptions_sdk::digest::compute_digest;
use ethscriptions_sdk::encode::{DataUriOptions, build_data_uri, detect_content_type};
use ethscriptions_sdk::validate::bytes_to_hex;
use serde::Serialize;

use crate::input::require_input;
use crate::output::print_json;

#[derive(Args)]
#[command(next_display_order = None)]
pub struct EncodeArgs {
    /// Data string to encode
    pub data: Option<String>,

    /// Read input from a file
    #[arg(long)]
    pub file: Option<PathBuf>,

    /// Output hex encoding
    #[arg(long)]
    pub hex: bool,

    /// Output base64 encoding
    #[arg(long)]
    pub base64: bool,

    /// Output data URI
    #[arg(long)]
    pub data_uri: bool,

    /// Omit the content type prefix from generated data URIs
    #[arg(long, visible_alias = "skip-type", requires = "data_uri")]
    pub no_content_type: bool,

    /// Output SHA-256 hash
    #[arg(long)]
    pub sha: bool,

    /// Print only the requested encoded value
    #[arg(long)]
    pub raw: bool,
}

#[derive(Serialize)]
struct EncodeOutput {
    content_type: String,
    size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_uri: Option<String>,
}

pub fn run(args: EncodeArgs, pretty: bool) -> Result<(), String> {
    let bytes = require_input(args.file.as_ref(), args.data.as_deref(), "encode")?;
    let content_type = detect_content_type(&bytes);
    let explicit_outputs = [args.sha, args.hex, args.base64, args.data_uri]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();
    if args.raw && explicit_outputs != 1 {
        return Err(
            "`--raw` requires exactly one explicit output flag like `--hex`, `--base64`, `--data-uri`, or `--sha`."
                .to_string(),
        );
    }

    if args.data_uri
        && args.file.is_none()
        && matches!(args.data.as_deref(), Some(data) if data.starts_with("data:"))
    {
        return Err(
            "`--data-uri` cannot be used when the direct input already starts with `data:`."
                .to_string(),
        );
    }

    let sha256 = if args.sha {
        Some(compute_digest(&bytes))
    } else {
        None
    };

    let hex = if args.hex {
        Some(bytes_to_hex(&bytes))
    } else {
        None
    };

    let base64 = if args.base64 {
        Some(base64::engine::general_purpose::STANDARD.encode(&bytes))
    } else {
        None
    };

    let data_uri = if args.data_uri {
        Some(build_data_uri(
            &bytes,
            &DataUriOptions {
                skip_content_type: args.no_content_type,
                esip6: false,
            },
        ))
    } else {
        None
    };

    if args.raw {
        let raw_value = sha256
            .clone()
            .or(hex.clone())
            .or(base64.clone())
            .or(data_uri.clone())
            .ok_or_else(|| {
                "`--raw` requires exactly one explicit output flag like `--hex`, `--base64`, `--data-uri`, or `--sha`."
                    .to_string()
            })?;
        print!("{raw_value}");
        return Ok(());
    }

    let output = EncodeOutput {
        content_type,
        size: bytes.len(),
        sha256,
        hex,
        base64,
        data_uri,
    };

    print_json(&output, pretty);
    Ok(())
}
