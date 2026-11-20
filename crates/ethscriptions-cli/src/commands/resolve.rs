// SPDX-License-Identifier: MPL-2.0

use clap::{Args, CommandFactory};
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::resolve;

use crate::Cli;
use crate::input::require_text_arg;
use crate::output::print_json;

#[derive(Args)]
#[command(next_display_order = None)]
pub struct ResolveArgs {
    /// ENS name, ethscription name, or Ethereum address to resolve (also accepts stdin)
    pub name: Option<String>,

    /// Also check if the resolved address is a creator
    #[arg(long)]
    pub check_creator: bool,

    /// Output full JSON instead of plain text address
    #[arg(long)]
    pub json: bool,
}

pub async fn run(client: &Client, args: ResolveArgs, pretty: bool) -> Result<(), String> {
    let name = match require_text_arg(args.name.as_deref(), "resolve") {
        Ok(val) => val,
        Err(_) => {
            let mut cmd = Cli::command();
            let sub = cmd
                .find_subcommand_mut("resolve")
                .expect("resolve subcommand");
            sub.print_help().map_err(|e| e.to_string())?;
            println!();
            return Ok(());
        }
    };

    let result = resolve::resolve_user(
        &client.http,
        &name,
        args.check_creator,
        Some(&client.api_url),
    )
    .await
    .map_err(|e| e.to_string())?;

    if args.json {
        print_json(&result, pretty);
    } else {
        println!("{}", result.address);
    }
    Ok(())
}
