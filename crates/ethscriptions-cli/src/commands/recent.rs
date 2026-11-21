// SPDX-License-Identifier: MPL-2.0

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::query::PulseQuery;

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

pub async fn run(client: &Client, args: RecentArgs, pretty: bool) -> Result<(), String> {
    let query = PulseQuery {
        from_address: args.from_address,
        to_address: args.to_address,
        to_or_from: args.to_or_from,
        transaction_hash: args.transaction_hash,
        token_tick: args.token_tick,
        token_protocol: args.token_protocol,
        sort_by: args.sort_by,
        reverse: args.reverse,
        page_size: args.page_size,
        page_key: args.page_key,
        resolve: true,
    };

    let result = client
        .fetch_recent_pulse(query)
        .await
        .map_err(|e| e.to_string())?;

    print_json(&result, pretty);
    Ok(())
}
