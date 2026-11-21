// SPDX-License-Identifier: MPL-2.0

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::fetcher::ContentData;
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

pub async fn run_content(client: &Client, args: ContentArgs) -> Result<(), String> {
    let raw = read_text_arg(args.id.as_deref())?
        .ok_or_else(|| "No ID provided. Pass as argument or pipe via stdin.".to_string())?;
    let id =
        parse_ethscription_id(&raw).ok_or_else(|| format!("Invalid ethscription ID: '{raw}'"))?;

    // Fetch the ethscription — use the raw upstream response which always has content_uri
    let record = client
        .fetch_ethscription(&id)
        .await
        .map_err(|e| e.to_string())?;

    let content_uri = record
        .raw
        .content_uri
        .as_deref()
        .ok_or_else(|| "Ethscription has no content URI".to_string())?;

    let (data, _content_type) = client
        .fetch_ethscription_content(content_uri, Some(args.base64), Some(args.ungzip))
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

pub async fn run_attachment(client: &Client, args: AttachmentArgs) -> Result<(), String> {
    let raw = read_text_arg(args.id.as_deref())?
        .ok_or_else(|| "No ID provided. Pass as argument or pipe via stdin.".to_string())?;
    let id =
        parse_ethscription_id(&raw).ok_or_else(|| format!("Invalid ethscription ID: '{raw}'"))?;

    let data = client
        .fetch_ethscription_attachment(&id, Some(args.base64), Some(args.ungzip))
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
