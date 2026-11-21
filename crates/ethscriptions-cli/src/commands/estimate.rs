// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::Args;
use ethscriptions_sdk::client::Client;

use crate::input::require_input;
use crate::output::print_json;

#[derive(Args)]
#[command(next_display_order = None)]
pub struct EstimateArgs {
    /// Data string to estimate cost for
    pub data: Option<String>,

    /// Read input from a file
    #[arg(long)]
    pub file: Option<PathBuf>,

    /// Buffer fee in wei to add to gas estimate
    #[arg(long, default_value = "0")]
    pub buffer_fee: u64,

    /// Gas speed: "normal" or "fast"
    #[arg(long, default_value = "normal")]
    pub speed: String,
}

pub async fn run(client: &Client, args: EstimateArgs, pretty: bool) -> Result<(), String> {
    let bytes = require_input(args.file.as_ref(), args.data.as_deref(), "estimate")?;
    let output = client
        .estimate_data_cost(&bytes, &args.speed, args.buffer_fee)
        .await
        .map_err(|e| e.to_string())?;

    print_json(&output, pretty);
    Ok(())
}
