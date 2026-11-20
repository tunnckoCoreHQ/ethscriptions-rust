// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::encode::InputSource;
use ethscriptions_sdk::inscribe::{self, InscribeOptions};

use crate::input::read_input;
use crate::output::print_json;

#[derive(Args)]
#[command(next_display_order = None)]
pub struct InscribeArgs {
    /// Literal content to inscribe
    pub data: Option<String>,

    /// Read inscription content from a file
    #[arg(long)]
    pub file: Option<PathBuf>,

    /// Use an EIP-4844 blob transaction
    #[arg(long)]
    pub blobs: bool,

    /// Prepare and validate without submitting the transaction
    #[arg(long)]
    pub dry_run: bool,

    /// Optimize PNG/JPEG input before inscription
    #[arg(long)]
    pub optimize: bool,

    /// Gzip file or stdin input before inscription
    #[arg(long)]
    pub gzip: bool,

    /// Allow duplicate content by marking the data URI with rule=esip6
    #[arg(long)]
    pub esip6: bool,

    /// Omit the content type prefix from the generated data URI
    #[arg(long, visible_alias = "skip-type")]
    pub no_content_type: bool,

    /// Private key to sign with
    #[arg(long, visible_alias = "secret-key")]
    pub private_key: Option<String>,

    /// Mnemonic phrase to sign with
    #[arg(long)]
    pub mnemonic: Option<String>,

    /// Load signer and RPC values from a .env-style file
    #[arg(long)]
    pub env_file: Option<String>,

    /// Ethereum RPC URL
    #[arg(long, env = "ETHS_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Recipient / initial owner address, defaults to self
    #[arg(long, visible_alias = "recipient", visible_alias = "initial-owner")]
    pub to: Option<String>,

    /// Chain ID to submit to
    #[arg(long, default_value_t = 1)]
    pub chain_id: u64,
}

pub async fn run(client: &Client, args: InscribeArgs, pretty: bool) -> Result<(), String> {
    let (bytes, source) = resolve_input(args.file.as_ref(), args.data.as_deref())?;
    let result = inscribe::inscribe(
        client,
        &InscribeOptions {
            bytes,
            source,
            optimize: args.optimize,
            gzip: args.gzip,
            esip6: args.esip6,
            skip_content_type: args.no_content_type,
            blobs: args.blobs,
            dry_run: args.dry_run,
            private_key: args.private_key,
            mnemonic: args.mnemonic,
            env_file: args.env_file,
            rpc_url: args.rpc_url,
            to: args.to,
            chain_id: args.chain_id,
        },
    )
    .await
    .map_err(|e| e.to_string())?;

    match result {
        inscribe::InscribeResult::Submitted(submitted) => print_json(&submitted, pretty),
        inscribe::InscribeResult::DryRun(dry_run) => print_json(&dry_run, pretty),
    }

    Ok(())
}

fn resolve_input(
    file: Option<&PathBuf>,
    data: Option<&str>,
) -> Result<(Vec<u8>, InputSource), String> {
    let bytes = read_input(file, data)?.ok_or_else(|| {
        "No input provided. Use --file <path>, pipe via stdin, or pass as argument.\nRun `eths inscribe --help` for usage.".to_string()
    })?;

    let source = if file.is_some() {
        InputSource::File
    } else if data.is_some() {
        InputSource::Literal
    } else {
        InputSource::Stdin
    };

    Ok((bytes, source))
}
