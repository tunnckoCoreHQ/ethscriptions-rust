// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use clap::Args;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::normalize::apply_field_selection_list;
use ethscriptions_sdk::types::{Ethscription, EthscriptionListResult, Pagination, RawEthscription};
use ethscriptions_sdk::validate::is_valid_address;
use serde::Serialize;

use crate::input::require_text_arg;
use crate::output::print_json;

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

pub async fn run(client: &Client, args: ProfileArgs, pretty: bool) -> Result<(), String> {
    let name = require_text_arg(args.name.as_deref(), "profile")?;

    if args.created || args.owned {
        let output = fetch_profile_list(client, &name, &args).await?;
        print_json(&output, pretty);
        return Ok(());
    }

    let list_result = fetch_profile_state(client, &name).await?;

    if args.info {
        print_profile_info(client, &list_result.raws, pretty).await?;
        return Ok(());
    }

    let output = build_profile_output(&list_result.result, &list_result.raws, &args, &name)?;
    print_json(&output, pretty);
    Ok(())
}

async fn fetch_profile_state(
    client: &Client,
    name: &str,
) -> Result<EthscriptionListResult, String> {
    let mut params = HashMap::new();
    params.insert("creator".to_string(), name.to_string());
    params.insert(
        "media_subtype".to_string(),
        "vnd.esc.user.profile+json".to_string(),
    );
    params.insert("media_type".to_string(), "application".to_string());

    if !is_valid_address(name) {
        params.insert("resolve".to_string(), "true".to_string());
    }

    let list_result = client
        .fetch_ethscriptions(&params)
        .await
        .map_err(|e| e.to_string())?;

    if list_result.result.is_empty() {
        return Err(format!("Not found: No user profile found for {name}"));
    }

    Ok(list_result)
}

async fn fetch_profile_list(
    client: &Client,
    name: &str,
    args: &ProfileArgs,
) -> Result<ListOutput<Vec<serde_json::Value>, Pagination>, String> {
    let mut params = HashMap::new();

    if let Some(page_size) = args.page_size {
        params.insert("page_size".to_string(), page_size.to_string());
    }

    if let Some(page_key) = &args.page_key {
        params.insert("page_key".to_string(), page_key.clone());
    }

    let list_result = if args.created {
        client
            .fetch_user_created_ethscriptions(name, &params)
            .await
            .map_err(|e| e.to_string())?
    } else {
        client
            .fetch_user_owned_ethscriptions(name, &params)
            .await
            .map_err(|e| e.to_string())?
    };

    let selected = apply_field_selection_list(
        &list_result.result,
        &list_result.raws,
        args.with_fields.as_deref(),
        args.only.as_deref(),
        None,
    );

    Ok(ListOutput {
        result: selected,
        pagination: list_result.pagination,
    })
}

fn build_profile_output(
    ethscriptions: &[Ethscription],
    raws: &[RawEthscription],
    args: &ProfileArgs,
    name: &str,
) -> Result<ProfileOutput<serde_json::Value>, String> {
    let with_fields = force_content_uri(args.with_fields.as_deref());
    let selected = apply_field_selection_list(
        ethscriptions,
        raws,
        with_fields.as_deref(),
        args.only.as_deref(),
        None,
    );

    let mut iter = selected.into_iter();
    let latest = iter
        .next()
        .ok_or_else(|| format!("Not found: No user profile found for {name}"))?;

    Ok(ProfileOutput {
        latest,
        previous: iter.collect(),
    })
}

async fn print_profile_info(
    client: &Client,
    raws: &[RawEthscription],
    pretty: bool,
) -> Result<(), String> {
    let latest = raws
        .first()
        .ok_or_else(|| "Not found: No user profile found".to_string())?;
    let content_uri = latest
        .content_uri
        .as_deref()
        .ok_or_else(|| "Profile ethscription is missing content_uri".to_string())?;

    let (content, _content_type) = client
        .fetch_ethscription_content(content_uri, Some(false), Some(false))
        .await
        .map_err(|e| e.to_string())?;
    let bytes = content.as_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Failed to parse profile content JSON: {e}"))?;

    print_json(&value, pretty);
    Ok(())
}

fn force_content_uri(with_fields: Option<&str>) -> Option<String> {
    match with_fields {
        Some(fields) if !fields.trim().is_empty() => {
            let mut items: Vec<&str> = fields
                .split(',')
                .map(str::trim)
                .filter(|field| !field.is_empty())
                .collect();
            if !items.contains(&"content_uri") {
                items.push("content_uri");
            }
            Some(items.join(","))
        }
        _ => Some("content_uri".to_string()),
    }
}
