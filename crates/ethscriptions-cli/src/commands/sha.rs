// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::digest::compute_name_digest;
use serde::Serialize;

use crate::input::require_input;
use crate::output::print_json;

#[derive(Args)]
#[command(next_display_order = None)]
pub struct ShaArgs {
    /// Data string to hash
    pub data: Option<String>,

    /// Read input from a file
    #[arg(long)]
    pub file: Option<PathBuf>,

    /// Compute digest for ethscription name (data:,{name})
    #[arg(long)]
    pub name: Option<String>,

    /// Output full JSON instead of plain text SHA
    #[arg(long)]
    pub json: bool,

    /// Omit the content type prefix from generated data URIs in JSON output
    #[arg(long, visible_alias = "skip-type")]
    pub no_content_type: bool,
}

#[derive(Serialize)]
struct ShaOutput {
    sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_uri: Option<String>,
}

/// Build a proper `data:{content_type};base64,{encoded}` URI from raw bytes,
/// reusing the same content-type detection logic as the `meta` command.
fn build_data_uri(bytes: &[u8], no_content_type: bool) -> String {
    ethscriptions_sdk::encode::build_data_uri(
        bytes,
        &ethscriptions_sdk::encode::DataUriOptions {
            skip_content_type: no_content_type,
            esip6: false,
        },
    )
}

pub async fn run(client: &Client, args: ShaArgs, pretty: bool) -> Result<(), String> {
    // --name shortcut: compute digest for `data:,{name}`
    if let Some(name) = &args.name {
        let sha = compute_name_digest(name);
        if args.json {
            let output = ShaOutput {
                sha,
                hex: None,
                data_uri: Some(format!("data:,{name}")),
            };
            print_json(&output, pretty);
        } else {
            print!("{sha}");
        }
        return Ok(());
    }

    let bytes = require_input(args.file.as_ref(), args.data.as_deref(), "sha")?;
    let source_data_uri = if args.file.is_none() {
        args.data
            .as_ref()
            .filter(|data| data.starts_with("data:"))
            .cloned()
    } else {
        None
    };
    let result = client
        .get_digest_for_data(&bytes, false)
        .await
        .map_err(|e| e.to_string())?;

    if args.json {
        let output = ShaOutput {
            sha: result.sha,
            hex: Some(result.hex),
            data_uri: Some(
                source_data_uri.unwrap_or_else(|| build_data_uri(&bytes, args.no_content_type)),
            ),
        };
        print_json(&output, pretty);
    } else {
        print!("{}", result.sha);
    }

    Ok(())
}
