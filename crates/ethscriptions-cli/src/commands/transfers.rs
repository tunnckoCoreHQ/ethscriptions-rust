// SPDX-License-Identifier: MPL-2.0

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::fetcher;
use ethscriptions_sdk::validate::parse_ethscription_id;

use crate::input::require_text_arg;
use crate::output::print_json;

#[derive(Args)]
#[command(next_display_order = None)]
pub struct TransfersArgs {
    /// Ethscription ID: transaction hash (0x...) or numeric number
    pub id: Option<String>,
}

pub async fn run(client: &Client, args: TransfersArgs, pretty: bool) -> Result<(), String> {
    let raw = require_text_arg(args.id.as_deref(), "transfers")?;
    let id =
        parse_ethscription_id(&raw).ok_or_else(|| format!("Invalid ethscription ID: '{raw}'"))?;

    let (_ethscription, raw_eth) = client
        .fetch_ethscription(&id)
        .await
        .map_err(|e| e.to_string())?;

    let transfers = fetcher::extract_transfers(&raw_eth);
    print_json(&transfers, pretty);
    Ok(())
}
