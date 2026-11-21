// SPDX-License-Identifier: MPL-2.0

use clap::Args;
use ethscriptions_sdk::client::Client;

use crate::output::{Error, Output};

#[derive(Args)]
#[command(next_display_order = None)]
pub struct GasArgs {
    /// Gas speed: "low", "average", or "high"
    #[arg(long, default_value = "average")]
    pub speed: String,
}

pub async fn run(client: &Client, args: GasArgs, pretty: bool) -> Result<Output, Error> {
    let result = client.fetch_gas_prices(&args.speed).await?;

    Ok(Output::json(&result, pretty))
}
