// SPDX-License-Identifier: MPL-2.0

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::validate::parse_ethscription_id;

use crate::input::Stdin;
use crate::output::{Error, Output};

#[derive(Args)]
#[command(next_display_order = None)]
pub struct TransfersArgs {
    /// Ethscription ID: transaction hash (0x...) or numeric number
    pub id: Option<String>,
}

pub async fn run(
    client: &Client,
    args: TransfersArgs,
    stdin: &mut Stdin,
    pretty: bool,
) -> Result<Output, Error> {
    let raw = stdin.require_text_arg(args.id.as_deref(), "transfers")?;
    let id =
        parse_ethscription_id(&raw).ok_or_else(|| format!("Invalid ethscription ID: '{raw}'"))?;

    let record = client.fetch_ethscription(&id).await?;

    let transfers = record.transfers();
    Ok(Output::json(&transfers, pretty))
}
