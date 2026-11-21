// SPDX-License-Identifier: MPL-2.0

use clap::{Args, CommandFactory};
use ethscriptions_sdk::client::Client;

use crate::Cli;
use crate::input::Stdin;
use crate::output::{Error, Output};

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

pub async fn run(
    client: &Client,
    args: ResolveArgs,
    stdin: &mut Stdin,
    pretty: bool,
) -> Result<Output, Error> {
    let name = match stdin.require_text_arg(args.name.as_deref(), "resolve") {
        Ok(val) => val,
        Err(_) => {
            let cmd = Cli::command();
            let sub = cmd.find_subcommand("resolve").expect("resolve subcommand");
            return Ok(Output::Help(Box::new(sub.clone())));
        }
    };

    let result = client.resolve_user(&name, args.check_creator).await?;

    if args.json {
        Ok(Output::json(&result, pretty))
    } else {
        Ok(Output::Text(format!("{}\n", result.address)))
    }
}
