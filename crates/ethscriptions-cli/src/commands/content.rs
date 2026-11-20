// SPDX-License-Identifier: MPL-2.0

use clap::Args;
use ethscriptions_sdk::fetcher::{self, ContentData};
use ethscriptions_sdk::validate::parse_ethscription_id;

use crate::input::read_text_arg;
use crate::output::{print_bytes, write_to_file};

#[derive(Args)]
#[command(next_display_order = None)]
pub struct ContentArgs {
    /// Ethscription ID: transaction hash (0x...) or numeric number
    pub id: Option<String>,

    /// Output as base64 encoded string instead of raw bytes
    #[arg(long)]
    pub base64: bool,

    /// Ungzip the fetched content before output
    #[arg(long)]
    pub ungzip: bool,

    /// Write output to a file instead of stdout
    #[arg(long, short)]
    pub output: Option<String>,
}

#[derive(Args)]
#[command(next_display_order = None)]
pub struct AttachmentArgs {
    /// Ethscription ID: transaction hash (0x...) or numeric number
    pub id: Option<String>,

    /// Output as base64 encoded string instead of raw bytes
    #[arg(long)]
    pub base64: bool,

    /// Ungzip the fetched attachment before output
    #[arg(long)]
    pub ungzip: bool,

    /// Write output to a file instead of stdout
    #[arg(long, short)]
    pub output: Option<String>,
}

pub async fn run_content(
    client: &reqwest::Client,
    args: ContentArgs,
    base_url: Option<&str>,
) -> Result<(), String> {
    let raw = read_text_arg(args.id.as_deref())?
        .ok_or_else(|| "No ID provided. Pass as argument or pipe via stdin.".to_string())?;
    let id =
        parse_ethscription_id(&raw).ok_or_else(|| format!("Invalid ethscription ID: '{raw}'"))?;

    // Fetch the ethscription — use the raw upstream response which always has content_uri
    let (_ethscription, raw_resp) = fetcher::fetch_ethscription(client, &id, base_url)
        .await
        .map_err(|e| e.to_string())?;

    let content_uri = raw_resp
        .content_uri
        .as_deref()
        .ok_or_else(|| "Ethscription has no content URI".to_string())?;

    let (data, _content_type) = fetcher::fetch_ethscription_content(
        client,
        content_uri,
        Some(args.base64),
        Some(args.ungzip),
    )
    .await
    .map_err(|e| e.to_string())?;

    match data {
        ContentData::Base64(encoded) => {
            if let Some(path) = &args.output {
                write_to_file(encoded.as_bytes(), path)?;
            } else {
                println!("{encoded}");
            }
        }
        ContentData::Bytes(bytes) => {
            if let Some(path) = &args.output {
                write_to_file(&bytes, path)?;
            } else {
                print_bytes(&bytes);
            }
        }
    }

    Ok(())
}

pub async fn run_attachment(
    client: &reqwest::Client,
    args: AttachmentArgs,
    base_url: Option<&str>,
) -> Result<(), String> {
    let raw = read_text_arg(args.id.as_deref())?
        .ok_or_else(|| "No ID provided. Pass as argument or pipe via stdin.".to_string())?;
    let id =
        parse_ethscription_id(&raw).ok_or_else(|| format!("Invalid ethscription ID: '{raw}'"))?;

    let data = fetcher::fetch_ethscription_attachment(
        client,
        &id,
        base_url,
        Some(args.base64),
        Some(args.ungzip),
    )
    .await
    .map_err(|e| e.to_string())?;

    match data {
        ContentData::Base64(encoded) => {
            if let Some(path) = &args.output {
                write_to_file(encoded.as_bytes(), path)?;
            } else {
                println!("{encoded}");
            }
        }
        ContentData::Bytes(bytes) => {
            if let Some(path) = &args.output {
                write_to_file(&bytes, path)?;
            } else {
                print_bytes(&bytes);
            }
        }
    }

    Ok(())
}
