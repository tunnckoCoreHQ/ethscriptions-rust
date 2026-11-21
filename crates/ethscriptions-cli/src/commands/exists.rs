// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::digest::compute_digest;
use ethscriptions_sdk::validate::is_valid_sha256;

use crate::input::Stdin;
use crate::output::{Error, Output};

#[derive(Args)]
#[command(next_display_order = None)]
pub struct ExistsArgs {
    /// SHA-256 hash to check, or data to hash (when using --file or stdin)
    pub value: Option<String>,

    /// Read input from a file (computes SHA-256 automatically)
    #[arg(long)]
    pub file: Option<PathBuf>,
}

pub async fn run(
    client: &Client,
    args: ExistsArgs,
    stdin: &mut Stdin,
    pretty: bool,
) -> Result<Output, Error> {
    let sha = resolve_sha(stdin, args.file.as_ref(), args.value.as_deref())?;

    let result = client.check_exists(&sha).await?;

    Ok(Output::json(&result, pretty))
}

/// Determine the SHA-256 to check:
/// - If --file is provided, read file bytes and compute SHA-256
/// - If piped stdin or positional arg is a valid SHA-256, use it directly
/// - If piped stdin or positional arg is not a valid SHA-256, treat as literal data
fn resolve_sha(
    stdin: &mut Stdin,
    file: Option<&PathBuf>,
    value: Option<&str>,
) -> Result<String, String> {
    // --file: always read as binary content and compute SHA
    if let Some(path) = file {
        let input = stdin
            .read_input(Some(path), None)?
            .ok_or_else(|| format!("Failed to read file '{}'", path.display()))?;
        return Ok(compute_digest(&input.bytes));
    }

    // Try reading a text value from positional arg or stdin
    let text = stdin.read_text_arg(value)?;
    match text {
        Some(val) if is_valid_sha256(&val) => Ok(val),
        Some(val) => {
            // Treat as literal data and compute SHA
            Ok(compute_digest(val.as_bytes()))
        }
        None => Err(
            "No input provided. Pass a SHA-256 hash, use --file <path>, or pipe via stdin.\nRun `eths exists --help` for usage.".to_string(),
        ),
    }
}
