// SPDX-License-Identifier: MPL-2.0

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::query::ListQuery;
use ethscriptions_sdk::types::FieldSelection;
use serde::Serialize;

use crate::output::print_json;

#[derive(Args)]
#[command(next_display_order = None)]
pub struct ListArgs {
    // -- Address filters --
    /// Filter by creator address
    #[arg(long)]
    pub creator: Option<String>,

    /// Filter by receiver / initial owner address
    #[arg(long, visible_alias = "initial-owner", visible_alias = "initial")]
    pub receiver: Option<String>,

    /// Filter by current owner address
    #[arg(long, visible_alias = "current", visible_alias = "owner")]
    pub current_owner: Option<String>,

    /// Filter by previous owner address
    #[arg(long, visible_alias = "previous")]
    pub previous_owner: Option<String>,

    // -- Content type filters --
    /// Filter by media type (e.g. "text", "image")
    #[arg(long)]
    pub media_type: Option<String>,

    /// Filter by media subtype (e.g. "plain", "png")
    #[arg(long, visible_alias = "mime-subtype")]
    pub media_subtype: Option<String>,

    /// Filter by content type / MIME type (e.g. "text/plain")
    #[arg(long, visible_alias = "mimetype")]
    pub content_type: Option<String>,

    // -- Content / hash filters --
    /// Filter by content SHA hash
    #[arg(long)]
    pub content_sha: Option<String>,

    // -- Block filters --
    /// Filter by block number
    #[arg(long)]
    pub block_number: Option<String>,

    /// Filter by block hash
    #[arg(long, visible_alias = "block-blockhash")]
    pub block_hash: Option<String>,

    /// Only return ethscriptions after this block number
    #[arg(long)]
    pub after_block: Option<u64>,

    /// Only return ethscriptions before this block number
    #[arg(long)]
    pub before_block: Option<u64>,

    // -- Attachment / ESIP-8 filters --
    /// Filter by attachment SHA hash
    #[arg(long)]
    pub attachment_sha: Option<String>,

    /// Filter by attachment content type
    #[arg(long)]
    pub attachment_content_type: Option<String>,

    /// Filter by presence of an attachment (ESIP-8)
    #[arg(long, visible_alias = "blobs", visible_alias = "blobscriptions")]
    pub attachment_present: bool,

    // -- Token filters --
    /// Filter by token tick (e.g. "eths")
    #[arg(long)]
    pub token_tick: Option<String>,

    /// Filter by token protocol (e.g. "erc-20")
    #[arg(long)]
    pub token_protocol: Option<String>,

    // -- Transfer filter --
    /// Filter by transfer transaction hash
    #[arg(long)]
    pub transferred_in_tx: Option<String>,

    // -- Response shape --
    /// Return only transaction hashes (increases limit to 1000)
    #[arg(long)]
    pub transaction_hash_only: bool,

    // -- Sorting --
    /// Sort order: "newest_first" (default) or "oldest_first"
    #[arg(long)]
    pub sort_by: Option<String>,

    /// Reverse the sort order
    #[arg(long)]
    pub reverse: bool,

    // -- Pagination --
    /// Number of results per page
    #[arg(long, visible_alias = "per-page", visible_alias = "max-results")]
    pub page_size: Option<u32>,

    /// Pagination cursor
    #[arg(long, visible_alias = "cursor")]
    pub page_key: Option<String>,

    // -- Field selection (SDK-only) --
    /// Include latest transfer info (latest_transfer_hash, latest_transfer_block)
    #[arg(long)]
    pub latest_transfer: bool,

    /// Only include these fields (comma-separated, e.g. "transaction_hash,creator")
    #[arg(long)]
    pub only: Option<String>,

    /// Extra fields to include (comma-separated, e.g. "content_uri,current_owner")
    #[arg(long = "with")]
    pub with_fields: Option<String>,

    /// Resolve ENS names and Ethscription names to addresses
    #[arg(long)]
    pub resolve: bool,
}

#[derive(Serialize)]
struct ListOutput<T: Serialize, P: Serialize> {
    result: T,
    pagination: Option<P>,
}

pub async fn run(client: &Client, args: ListArgs, pretty: bool) -> Result<(), String> {
    let query = ListQuery {
        creator: args.creator,
        receiver: args.receiver,
        current_owner: args.current_owner,
        previous_owner: args.previous_owner,
        media_type: args.media_type,
        media_subtype: args.media_subtype,
        content_type: args.content_type,
        content_sha: args.content_sha,
        block_number: args.block_number,
        block_hash: args.block_hash,
        after_block: args.after_block,
        before_block: args.before_block,
        attachment_sha: args.attachment_sha,
        attachment_content_type: args.attachment_content_type,
        attachment_present: args.attachment_present,
        token_tick: args.token_tick,
        token_protocol: args.token_protocol,
        transferred_in_tx: args.transferred_in_tx,
        transaction_hash_only: args.transaction_hash_only,
        sort_by: args.sort_by,
        reverse: args.reverse,
        page_size: args.page_size,
        page_key: args.page_key,
        resolve: args.resolve,
    };

    let list_result = client
        .fetch_ethscriptions(query)
        .await
        .map_err(|e| e.to_string())?;

    let has_field_selection =
        args.with_fields.is_some() || args.only.is_some() || args.latest_transfer;

    if has_field_selection {
        let selection = FieldSelection::new(
            args.with_fields.as_deref(),
            args.only.as_deref(),
            args.latest_transfer,
        );
        let output = ListOutput {
            result: list_result
                .records
                .iter()
                .map(|record| record.select(&selection))
                .collect::<Vec<_>>(),
            pagination: list_result.pagination,
        };
        print_json(&output, pretty);
    } else {
        let output = ListOutput {
            result: list_result
                .records
                .into_iter()
                .map(|record| record.normalized)
                .collect::<Vec<_>>(),
            pagination: list_result.pagination,
        };
        print_json(&output, pretty);
    }

    Ok(())
}
