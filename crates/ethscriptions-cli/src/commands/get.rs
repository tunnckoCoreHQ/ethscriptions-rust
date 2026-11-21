// SPDX-License-Identifier: MPL-2.0

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::types::FieldSelection;
use ethscriptions_sdk::validate::parse_ethscription_id;

use crate::input::require_text_arg;
use crate::output::print_json;

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

pub async fn run(client: &Client, args: GetArgs, pretty: bool) -> Result<(), String> {
    let raw = require_text_arg(args.id.as_deref(), "get")?;
    let id =
        parse_ethscription_id(&raw).ok_or_else(|| format!("Invalid ethscription ID: '{raw}'"))?;

    if args.owners {
        let record = client
            .fetch_ethscription_uncached(&id)
            .await
            .map_err(|e| e.to_string())?;
        print_json(&record.owners(), pretty);
        return Ok(());
    }

    let record = client
        .fetch_ethscription(&id)
        .await
        .map_err(|e| e.to_string())?;

    if args.numbers {
        print_json(&record.numbers(), pretty);
    } else {
        let selection = FieldSelection::new(
            args.with_fields.as_deref(),
            args.only.as_deref(),
            args.latest_transfer,
        );
        print_json(&record.select(&selection), pretty);
    }

    Ok(())
}
