// SPDX-License-Identifier: MPL-2.0

use clap::Args;
use ethscriptions_sdk::client::Client;

use crate::output::print_json;

#[derive(Args)]
#[command(next_display_order = None)]
pub struct GasArgs {
    /// Gas speed: "low", "average", or "high"
    #[arg(long, default_value = "average")]
    pub speed: String,
}

pub async fn run(client: &Client, args: GasArgs, pretty: bool) -> Result<(), String> {
    let result = client
        .fetch_gas_prices(&args.speed)
        .await
        .map_err(|e| e.to_string())?;

    print_json(&result, pretty);
    Ok(())
}
