// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::Args;
use ethscriptions_sdk::client::Client;

use crate::input::Stdin;
use crate::output::{Error, Output};

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

pub async fn run(
    client: &Client,
    args: EstimateArgs,
    stdin: &mut Stdin,
    pretty: bool,
) -> Result<Output, Error> {
    let bytes = stdin
        .require_input(args.file.as_ref(), args.data.as_deref(), "estimate")?
        .bytes;
    let output = client
        .estimate_data_cost(&bytes, &args.speed, args.buffer_fee)
        .await?;

    Ok(Output::json(&output, pretty))
}
