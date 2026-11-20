// SPDX-License-Identifier: MPL-2.0

mod commands;
mod input;
mod output;

use clap::{Parser, Subcommand};
use ethscriptions_sdk::client::Client;

/// Ethscriptions CLI — interact with the Ethscriptions protocol from your terminal.
#[derive(Parser)]
#[command(name = "eths", version, about, long_about = None, next_display_order = None)]
pub struct Cli {
    /// Output JSON in pretty-printed format
    #[arg(long, global = true)]
    pub pretty: bool,

    /// Override the upstream API base URL
    #[arg(long, global = true, env = "ETHS_BASE_URL")]
    pub base_url: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Fetch a single ethscription by transaction hash or number
    Get(commands::get::GetArgs),

    /// List ethscriptions with optional filters
    List(Box<commands::list::ListArgs>),

    /// Check if content exists as an ethscription (by SHA or file/stdin)
    Exists(commands::exists::ExistsArgs),

    /// Download ethscription content
    Content(commands::content::ContentArgs),

    /// Download ethscription attachment
    Attachment(commands::content::AttachmentArgs),

    /// Resolve an ENS name, ethscription name, or address
    Resolve(commands::resolve::ResolveArgs),

    /// Show the recent global pulse feed of creations and transfers
    Recent(commands::recent::RecentArgs),

    /// List transfer history for an ethscription
    Transfers(commands::transfers::TransfersArgs),

    /// Compute SHA-256 digest of data
    Sha(commands::sha::ShaArgs),

    /// Inscribe content using calldata or EIP-4844 blobs
    Inscribe(commands::inscribe::InscribeArgs),

    /// Fetch current gas prices
    Gas(commands::gas::GasArgs),

    /// Estimate ethscription creation cost
    Estimate(commands::estimate::EstimateArgs),

    /// Encode input into hashes, hex, base64, or data URIs
    Encode(commands::encode::EncodeArgs),

    /// Fetch a user's profile state or created/owned ethscriptions
    Profile(commands::profile::ProfileArgs),

    /// Validate addresses, hashes, hex strings, and more
    Validate(commands::validate::ValidateArgs),
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let client = Client::new(cli.base_url.as_deref());
    let pretty = cli.pretty;

    let result = match cli.command {
        Commands::Get(args) => commands::get::run(&client, args, pretty).await,
        Commands::List(args) => commands::list::run(&client, *args, pretty).await,
        Commands::Exists(args) => commands::exists::run(&client, args, pretty).await,
        Commands::Content(args) => commands::content::run_content(&client, args).await,
        Commands::Attachment(args) => commands::content::run_attachment(&client, args).await,
        Commands::Resolve(args) => commands::resolve::run(&client, args, pretty).await,
        Commands::Recent(args) => commands::recent::run(&client, args, pretty).await,
        Commands::Transfers(args) => commands::transfers::run(&client, args, pretty).await,
        Commands::Sha(args) => commands::sha::run(&client, args, pretty).await,
        Commands::Inscribe(args) => commands::inscribe::run(&client, args, pretty).await,
        Commands::Gas(args) => commands::gas::run(&client, args, pretty).await,
        Commands::Estimate(args) => commands::estimate::run(&client, args, pretty).await,
        Commands::Encode(args) => commands::encode::run(args, pretty),
        Commands::Profile(args) => commands::profile::run(&client, args, pretty).await,
        Commands::Validate(args) => commands::validate::run(args, pretty),
    };

    if let Err(msg) = result {
        output::die(&msg);
    }
}
