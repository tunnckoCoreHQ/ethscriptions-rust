// SPDX-License-Identifier: MPL-2.0

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::query::ListQuery;
use ethscriptions_sdk::types::{FieldSelection, Pagination, RawEthscription};
use serde::Serialize;

use crate::input::Stdin;
use crate::output::{Error, Output};

#[derive(Serialize)]
struct ListOutput<T: Serialize, P: Serialize> {
    result: T,
    pagination: Option<P>,
}

#[derive(Serialize)]
struct ProfileOutput<T: Serialize> {
    latest: T,
    previous: Vec<T>,
}

#[derive(Args)]
#[command(next_display_order = None)]
pub struct ProfileArgs {
    /// ENS name, ethscription name, or Ethereum address to fetch profile for
    pub name: Option<String>,

    /// Show ethscriptions created by this profile instead of profile state
    #[arg(long, group = "mode")]
    pub created: bool,

    /// Show ethscriptions currently owned by this profile instead of profile state
    #[arg(long, group = "mode")]
    pub owned: bool,

    /// Return the decoded JSON content of the latest profile ethscription
    #[arg(long, group = "mode")]
    pub info: bool,

    /// Number of results per page for --created/--owned
    #[arg(long, visible_alias = "per-page")]
    pub page_size: Option<u32>,

    /// Pagination cursor for --created/--owned
    #[arg(long, visible_alias = "cursor")]
    pub page_key: Option<String>,

    /// Only include these fields (comma-separated)
    #[arg(long)]
    pub only: Option<String>,

    /// Extra fields to include (comma-separated)
    #[arg(long = "with")]
    pub with_fields: Option<String>,
}

pub async fn run(
    client: &Client,
    args: ProfileArgs,
    stdin: &mut Stdin,
    pretty: bool,
) -> Result<Output, Error> {
    let name = stdin.require_text_arg(args.name.as_deref(), "profile")?;

    if args.created || args.owned {
        let output = fetch_profile_list(client, &name, &args).await?;
        return Ok(Output::json(&output, pretty));
    }

    let profile = client.fetch_user_profile(&name).await?;

    if args.info {
        return profile_info(client, &profile.latest.raw, pretty).await;
    }

    let selection = field_selection(&args).with_field("content_uri");
    let output = ProfileOutput {
        latest: profile.latest.select(&selection),
        previous: profile
            .previous
            .iter()
            .map(|record| record.select(&selection))
            .collect(),
    };
    Ok(Output::json(&output, pretty))
}

async fn fetch_profile_list(
    client: &Client,
    name: &str,
    args: &ProfileArgs,
) -> Result<ListOutput<Vec<serde_json::Value>, Pagination>, Error> {
    let query = ListQuery {
        page_size: args.page_size,
        page_key: args.page_key.clone(),
        ..ListQuery::default()
    };

    let list_result = if args.created {
        client.fetch_user_created_ethscriptions(name, query).await?
    } else {
        client.fetch_user_owned_ethscriptions(name, query).await?
    };

    let selection = field_selection(args);
    Ok(ListOutput {
        result: list_result
            .records
            .iter()
            .map(|record| record.select(&selection))
            .collect(),
        pagination: list_result.pagination,
    })
}

fn field_selection(args: &ProfileArgs) -> FieldSelection {
    FieldSelection::new(args.with_fields.as_deref(), args.only.as_deref(), false)
}

async fn profile_info(
    client: &Client,
    latest: &RawEthscription,
    pretty: bool,
) -> Result<Output, Error> {
    let content_uri = latest
        .content_uri
        .as_deref()
        .ok_or_else(|| "Profile ethscription is missing content_uri".to_string())?;

    let (content, _content_type) = client
        .fetch_ethscription_content(content_uri, Some(false), Some(false))
        .await?;
    let bytes = content.as_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Failed to parse profile content JSON: {e}"))?;

    Ok(Output::json(&value, pretty))
}
