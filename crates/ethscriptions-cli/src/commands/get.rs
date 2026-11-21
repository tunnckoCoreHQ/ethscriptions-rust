// SPDX-License-Identifier: MPL-2.0

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::types::FieldSelection;
use ethscriptions_sdk::validate::parse_ethscription_id;

use crate::input::Stdin;
use crate::output::{Error, Output};

#[derive(Args)]
#[command(next_display_order = None)]
pub struct GetArgs {
    /// Ethscription ID: transaction hash (0x...) or numeric number
    pub id: Option<String>,

    /// Include latest transfer info (latest_transfer_hash, latest_transfer_block)
    #[arg(long)]
    pub latest_transfer: bool,

    /// Fetch and display numbering and statistics
    #[arg(long, group = "mode")]
    pub numbers: bool,

    /// Only include these fields (comma-separated, e.g. "transaction_hash,creator")
    #[arg(long)]
    pub only: Option<String>,

    /// Fetch and display ownership history
    #[arg(long, group = "mode")]
    pub owners: bool,

    /// Extra fields to include (comma-separated, e.g. "content_uri,current_owner")
    #[arg(long = "with")]
    pub with_fields: Option<String>,
}

pub async fn run(
    client: &Client,
    args: GetArgs,
    stdin: &mut Stdin,
    pretty: bool,
) -> Result<Output, Error> {
    let raw = stdin.require_text_arg(args.id.as_deref(), "get")?;
    let id =
        parse_ethscription_id(&raw).ok_or_else(|| format!("Invalid ethscription ID: '{raw}'"))?;

    if args.owners {
        let record = client.fetch_ethscription_uncached(&id).await?;
        return Ok(Output::json(&record.owners(), pretty));
    }

    let record = client.fetch_ethscription(&id).await?;

    if args.numbers {
        Ok(Output::json(&record.numbers(), pretty))
    } else {
        let selection = FieldSelection::new(
            args.with_fields.as_deref(),
            args.only.as_deref(),
            args.latest_transfer,
        );
        Ok(Output::json(&record.select(&selection), pretty))
    }
}
