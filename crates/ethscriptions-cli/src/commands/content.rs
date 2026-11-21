// SPDX-License-Identifier: MPL-2.0

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::fetcher::ContentData;
use ethscriptions_sdk::validate::parse_ethscription_id;

use crate::input::Stdin;
use crate::output::{Error, Output};

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
    client: &Client,
    args: ContentArgs,
    stdin: &mut Stdin,
) -> Result<Output, Error> {
    let raw = stdin
        .read_text_arg(args.id.as_deref())?
        .ok_or_else(|| "No ID provided. Pass as argument or pipe via stdin.".to_string())?;
    let id =
        parse_ethscription_id(&raw).ok_or_else(|| format!("Invalid ethscription ID: '{raw}'"))?;

    // Fetch the ethscription — use the raw upstream response which always has content_uri
    let record = client.fetch_ethscription(&id).await?;

    let content_uri = record
        .raw
        .content_uri
        .as_deref()
        .ok_or_else(|| "Ethscription has no content URI".to_string())?;

    let (data, _content_type) = client
        .fetch_ethscription_content(content_uri, Some(args.base64), Some(args.ungzip))
        .await?;

    Ok(content_output(data, args.output))
}

pub async fn run_attachment(
    client: &Client,
    args: AttachmentArgs,
    stdin: &mut Stdin,
) -> Result<Output, Error> {
    let raw = stdin
        .read_text_arg(args.id.as_deref())?
        .ok_or_else(|| "No ID provided. Pass as argument or pipe via stdin.".to_string())?;
    let id =
        parse_ethscription_id(&raw).ok_or_else(|| format!("Invalid ethscription ID: '{raw}'"))?;

    let data = client
        .fetch_ethscription_attachment(&id, Some(args.base64), Some(args.ungzip))
        .await?;

    Ok(content_output(data, args.output))
}

fn content_output(data: ContentData, path: Option<String>) -> Output {
    match (data, path) {
        (ContentData::Base64(encoded), Some(path)) => Output::File {
            path,
            bytes: encoded.into_bytes(),
        },
        (ContentData::Base64(encoded), None) => Output::Text(format!("{encoded}\n")),
        (ContentData::Bytes(bytes), Some(path)) => Output::File { path, bytes },
        (ContentData::Bytes(bytes), None) => Output::Bytes(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_gets_a_newline_on_stdout_only() {
        let stdout = content_output(ContentData::Base64("aGk=".into()), None);
        assert!(matches!(stdout, Output::Text(text) if text == "aGk=\n"));
        let file = content_output(ContentData::Base64("aGk=".into()), Some("out".into()));
        assert!(matches!(file, Output::File { path, bytes } if path == "out" && bytes == b"aGk="));
    }

    #[test]
    fn bytes_go_to_stdout_or_the_file() {
        let stdout = content_output(ContentData::Bytes(vec![0, 255]), None);
        assert!(matches!(stdout, Output::Bytes(bytes) if bytes == [0, 255]));
        let file = content_output(ContentData::Bytes(vec![0, 255]), Some("out".into()));
        assert!(matches!(file, Output::File { bytes, .. } if bytes == [0, 255]));
    }
}
