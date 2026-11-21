// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::digest::compute_name_digest;
use serde::Serialize;

use crate::input::Stdin;
use crate::output::{Error, Output};

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

pub async fn run(
    client: &Client,
    args: ShaArgs,
    stdin: &mut Stdin,
    pretty: bool,
) -> Result<Output, Error> {
    // --name shortcut: compute digest for `data:,{name}`
    if let Some(name) = &args.name {
        let sha = compute_name_digest(name);
        if args.json {
            let output = ShaOutput {
                sha,
                hex: None,
                data_uri: Some(format!("data:,{name}")),
            };
            return Ok(Output::json(&output, pretty));
        }
        return Ok(Output::Text(sha));
    }

    let bytes = stdin
        .require_input(args.file.as_ref(), args.data.as_deref(), "sha")?
        .bytes;
    let source_data_uri = if args.file.is_none() {
        args.data
            .as_ref()
            .filter(|data| data.starts_with("data:"))
            .cloned()
    } else {
        None
    };
    let result = client.get_digest_for_data(&bytes, false).await?;

    if args.json {
        let output = ShaOutput {
            sha: result.sha,
            hex: Some(result.hex),
            data_uri: Some(
                source_data_uri.unwrap_or_else(|| build_data_uri(&bytes, args.no_content_type)),
            ),
        };
        Ok(Output::json(&output, pretty))
    } else {
        Ok(Output::Text(result.sha))
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use ethscriptions_sdk::digest::compute_digest;

    use super::*;
    use crate::{Cli, Commands};

    async fn sha(argv: &[&str], mut stdin: Stdin) -> Output {
        let Commands::Sha(args) = Cli::parse_from(argv).command else {
            panic!("not a sha command");
        };
        run(&Client::new(None), args, &mut stdin, false)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn name_digest_has_no_newline() {
        let output = sha(&["eths", "sha", "--name", "wgw"], Stdin::terminal()).await;
        assert!(matches!(output, Output::Text(text) if text == compute_name_digest("wgw")));
    }

    #[tokio::test]
    async fn json_data_uri_comes_from_the_arg_even_when_stdin_wins() {
        let output = sha(
            &["eths", "sha", "--json", "data:,x"],
            Stdin::piped(b"other"),
        )
        .await;
        let expected = format!(
            "{{\"data_uri\":\"data:,x\",\"hex\":\"6f74686572\",\"sha\":\"{}\"}}\n",
            compute_digest(b"other")
        );
        assert!(matches!(output, Output::Text(text) if text == expected));
    }
}
