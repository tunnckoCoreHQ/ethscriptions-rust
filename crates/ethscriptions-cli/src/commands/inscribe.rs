// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::encode::InputSource;
use ethscriptions_sdk::error::ApiError;
use ethscriptions_sdk::inscribe::{self, InscribeOptions, PrepareInscribeOptions};

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

const DEFAULT_RPC_URL: &str = "https://ethereum-rpc.publicnode.com";

pub async fn run(client: &Client, args: InscribeArgs, pretty: bool) -> Result<(), String> {
    let (bytes, source) = resolve_input(args.file.as_ref(), args.data.as_deref())?;

    let env_file = match &args.env_file {
        Some(path) => read_env_file(path)?,
        None => HashMap::new(),
    };
    let resolve = |flag: Option<String>, key: &str| {
        flag.or_else(|| std::env::var(key).ok())
            .or_else(|| env_file.get(key).cloned())
    };
    let private_key = resolve(args.private_key, "ETHS_PRIVATE_KEY");
    let mnemonic = resolve(args.mnemonic, "ETHS_MNEMONIC");
    let rpc_url = resolve(args.rpc_url, "ETHS_RPC_URL");

    let signer = match (private_key, mnemonic) {
        (Some(private_key), _) => inscribe::private_key_signer(&private_key),
        (None, Some(mnemonic)) => inscribe::mnemonic_signer(&mnemonic),
        (None, None) => Err(ApiError::bad_request(
            "Missing signer. Pass private_key or mnemonic, or set ETHS_PRIVATE_KEY / ETHS_MNEMONIC.",
        )),
    }
    .map_err(|e| e.to_string())?;

    let result = inscribe::inscribe(
        client,
        signer,
        rpc_url.as_deref().unwrap_or(DEFAULT_RPC_URL),
        &InscribeOptions {
            bytes,
            prepare: PrepareInscribeOptions {
                source,
                optimize: args.optimize,
                gzip: args.gzip,
                esip6: args.esip6,
                skip_content_type: args.no_content_type,
                ..Default::default()
            },
            blobs: args.blobs,
            dry_run: args.dry_run,
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

fn read_env_file(path: &str) -> Result<HashMap<String, String>, String> {
    let content = fs::read_to_string(path).map_err(|e| {
        ApiError::bad_request(format!("Failed to read env file '{path}': {e}")).to_string()
    })?;
    let mut values = HashMap::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(
                key.trim().to_string(),
                strip_quotes(value.trim()).to_string(),
            );
        }
    }

    Ok(values)
}

fn strip_quotes(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}
