// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use clap::Args;

use crate::output::print_json;

#[derive(Args)]
#[command(next_display_order = None)]
pub struct RecentArgs {
    /// Filter pulse events by sender address or resolvable name
    #[arg(long, visible_alias = "from")]
    pub from_address: Option<String>,

    /// Filter pulse events by recipient address or resolvable name
    #[arg(long, visible_alias = "to")]
    pub to_address: Option<String>,

    /// Filter pulse events by either sender or recipient address or resolvable name
    #[arg(long)]
    pub to_or_from: Option<String>,

    /// Filter pulse events by Ethereum transaction hash
    #[arg(long, visible_alias = "tx-hash", visible_alias = "tx")]
    pub transaction_hash: Option<String>,

    /// Filter pulse events by token tick
    #[arg(long)]
    pub token_tick: Option<String>,

    /// Filter pulse events by token protocol
    #[arg(long)]
    pub token_protocol: Option<String>,

    /// Sort order: newest_first or oldest_first
    #[arg(long)]
    pub sort_by: Option<String>,

    /// Reverse the sort order
    #[arg(long)]
    pub reverse: bool,

    /// Number of results per page
    #[arg(long, visible_alias = "per-page", visible_alias = "max-results")]
    pub page_size: Option<u32>,

    /// Pagination cursor
    #[arg(long, visible_alias = "cursor")]
    pub page_key: Option<String>,
}

fn insert_str(params: &mut HashMap<String, String>, key: &str, val: &Option<String>) {
    if let Some(v) = val {
        params.insert(key.to_string(), v.clone());
    }
}

fn insert_bool(params: &mut HashMap<String, String>, key: &str, val: bool) {
    if val {
        params.insert(key.to_string(), "true".to_string());
    }
}

fn insert_u32(params: &mut HashMap<String, String>, key: &str, val: Option<u32>) {
    if let Some(v) = val {
        params.insert(key.to_string(), v.to_string());
    }
}

pub async fn run(
    client: &reqwest::Client,
    args: RecentArgs,
    base_url: Option<&str>,
    pretty: bool,
) -> Result<(), String> {
    let mut params = HashMap::new();
    insert_str(&mut params, "from_address", &args.from_address);
    insert_str(&mut params, "to_address", &args.to_address);
    insert_str(&mut params, "to_or_from", &args.to_or_from);
    insert_str(&mut params, "transaction_hash", &args.transaction_hash);
    insert_str(&mut params, "ethscription_token_tick", &args.token_tick);
    insert_str(
        &mut params,
        "ethscription_token_protocol",
        &args.token_protocol,
    );
    insert_str(&mut params, "sort_by", &args.sort_by);
    insert_bool(&mut params, "reverse", args.reverse);
    insert_u32(&mut params, "page_size", args.page_size);
    insert_str(&mut params, "page_key", &args.page_key);
    insert_bool(&mut params, "resolve", true);

    let result = ethscriptions_sdk::fetcher::fetch_recent_pulse(client, &params, base_url)
        .await
        .map_err(|e| e.to_string())?;

    print_json(&result, pretty);
    Ok(())
}
