// SPDX-License-Identifier: MPL-2.0

use crate::client::Client;
use crate::digest::compute_name_digest;
use crate::error::ApiError;
use crate::types::{EnsDataResponse, ResolveUserResult, UpstreamExistsResponse};
use crate::validate::is_valid_address;

impl Client {
    /// Resolve a user identifier (ENS name, Ethscription Name, or address) to an address and name.
    ///
    /// If `val` is an address, performs reverse resolution (address -> name).
    /// If `val` is a name, resolves to an address.
    pub async fn resolve_user(
        &self,
        val: &str,
        check_creator: bool,
    ) -> Result<ResolveUserResult, ApiError> {
        let resolve_name = is_valid_address(val);

        let resolved = self
            .names_resolver(val, resolve_name, false, check_creator)
            .await;

        match resolved {
            Some(resolved_val) => {
                if resolve_name {
                    Ok(ResolveUserResult {
                        address: val.to_string(),
                        name: resolved_val,
                    })
                } else {
                    Ok(ResolveUserResult {
                        address: resolved_val,
                        name: val.to_string(),
                    })
                }
            }
            None => Err(ApiError::not_found(format!("Cannot resolve {val}"))),
        }
    }

    /// Core name resolution logic.
    ///
    /// 1. Try ENS resolution via ensdata.net API
    /// 2. Fall back to Ethscription Name resolution (SHA-256 of `data:,{name}`)
    pub async fn names_resolver(
        &self,
        value: &str,
        resolve_name: bool,
        primary: bool,
        check_creator: bool,
    ) -> Option<String> {
        let val = value.to_lowercase();

        // Try ENS resolution first
        if let Some(ens_data) = self.ens_api_handler(&val).await
            && let (Some(address), Some(ens)) = (&ens_data.address, &ens_data.ens)
            && !address.is_empty()
            && !ens.is_empty()
        {
            return Some(if resolve_name {
                if primary {
                    ens_data
                        .ens_primary
                        .unwrap_or_else(|| ens.clone())
                        .to_lowercase()
                } else {
                    ens.to_lowercase()
                }
            } else {
                address.to_lowercase()
            });
        }

        // Fall back to Ethscription Name resolution
        // Use the original value (not lowercased) for the name URI, as the SHA is case-sensitive
        let name_sha = compute_name_digest(value);
        let url = format!("{}/ethscriptions/exists/0x{name_sha}", self.api_url);

        let data: UpstreamExistsResponse = self.get_json(&url).await.ok()?;

        if data.result.exists
            && let Some(ref eth) = data.result.ethscription
        {
            let owner = if check_creator {
                eth.creator.clone().unwrap_or_default()
            } else {
                eth.current_owner.clone().unwrap_or_default()
            };
            return Some(owner.to_lowercase());
        }

        None
    }

    /// Call the ENS data API at `https://api.ensdata.net/{val}`.
    async fn ens_api_handler(&self, val: &str) -> Option<EnsDataResponse> {
        let url = format!("{}/{val}", self.ens_url);
        self.get_json(&url).await.ok()
    }

    /// Resolve address fields in filter parameters.
    ///
    /// For any filter param matching `creator|receiver|owner` whose value is not an address,
    /// attempt to resolve the name to an address.
    pub async fn resolve_address_patches(
        &self,
        params: &mut std::collections::HashMap<String, String>,
    ) {
        let address_keys = collect_resolvable_params(params);

        for (key, value) in address_keys {
            if let Some(resolved) = self.names_resolver(&value, false, false, false).await {
                params.insert(key, resolved);
            }
        }
    }
}

/// Check if a filter key is an address field that should be resolved.
pub fn is_address_filter_key(key: &str) -> bool {
    key.contains("creator")
        || key.contains("from_address")
        || key.contains("to_address")
        || key.contains("to_or_from")
        || key.contains("receiver")
        || key.contains("owner")
        || key.contains("initial")
        || key.contains("previous")
        || key.contains("current")
}

/// Collect filter entries whose keys are address fields and whose values need resolution.
///
/// Returns `(key, value)` pairs where the value is non-empty and not already a valid address.
pub fn collect_resolvable_params(
    params: &std::collections::HashMap<String, String>,
) -> Vec<(String, String)> {
    params
        .iter()
        .filter(|(key, value)| {
            is_address_filter_key(key) && !value.is_empty() && !is_valid_address(value)
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_is_address_filter_key_matches() {
        assert_eq!(is_address_filter_key("creator"), true);
        assert_eq!(is_address_filter_key("from_address"), true);
        assert_eq!(is_address_filter_key("to_address"), true);
        assert_eq!(is_address_filter_key("to_or_from"), true);
        assert_eq!(is_address_filter_key("receiver"), true);
        assert_eq!(is_address_filter_key("current_owner"), true);
        assert_eq!(is_address_filter_key("previous_owner"), true);
        assert_eq!(is_address_filter_key("owner"), true);
        assert_eq!(is_address_filter_key("current"), true);
        assert_eq!(is_address_filter_key("previous"), true);
        assert_eq!(is_address_filter_key("initial"), true);
    }

    #[test]
    fn test_is_address_filter_key_rejects() {
        assert_eq!(is_address_filter_key("media_type"), false);
        assert_eq!(is_address_filter_key("content_sha"), false);
        assert_eq!(is_address_filter_key("per_page"), false);
        assert_eq!(is_address_filter_key("block_number"), false);
        assert_eq!(is_address_filter_key("sort_by"), false);
        assert_eq!(is_address_filter_key("transaction_hash"), false);
        assert_eq!(is_address_filter_key("resolve"), false);
    }

    #[test]
    fn test_collect_resolvable_skips_valid_addresses() {
        let mut params = HashMap::new();
        params.insert(
            "creator".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        );
        params.insert("receiver".to_string(), "vitalik.eth".to_string());

        let resolvable = collect_resolvable_params(&params);
        assert_eq!(resolvable.len(), 1);
        assert_eq!(resolvable[0].0, "receiver");
        assert_eq!(resolvable[0].1, "vitalik.eth");
    }

    #[test]
    fn test_collect_resolvable_skips_empty_values() {
        let mut params = HashMap::new();
        params.insert("creator".to_string(), String::new());
        params.insert("receiver".to_string(), "somename".to_string());

        let resolvable = collect_resolvable_params(&params);
        assert_eq!(resolvable.len(), 1);
        assert_eq!(resolvable[0].0, "receiver");
    }

    #[test]
    fn test_collect_resolvable_skips_non_address_fields() {
        let mut params = HashMap::new();
        params.insert("media_type".to_string(), "text".to_string());
        params.insert("per_page".to_string(), "25".to_string());
        params.insert("creator".to_string(), "wgw.eth".to_string());

        let resolvable = collect_resolvable_params(&params);
        assert_eq!(resolvable.len(), 1);
        assert_eq!(resolvable[0].0, "creator");
    }

    #[test]
    fn test_collect_resolvable_multiple_fields() {
        let mut params = HashMap::new();
        params.insert("creator".to_string(), "wgw.eth".to_string());
        params.insert("receiver".to_string(), "tunnckocore".to_string());
        params.insert(
            "current_owner".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        );
        params.insert("previous_owner".to_string(), "vitalik.eth".to_string());

        let mut resolvable = collect_resolvable_params(&params);
        resolvable.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(resolvable.len(), 3);
        assert_eq!(resolvable[0].0, "creator");
        assert_eq!(resolvable[1].0, "previous_owner");
        assert_eq!(resolvable[2].0, "receiver");
    }

    #[test]
    fn test_collect_resolvable_empty_params() {
        let params = HashMap::new();
        let resolvable = collect_resolvable_params(&params);
        assert_eq!(resolvable.is_empty(), true);
    }

    #[test]
    fn test_collect_resolvable_all_valid_addresses() {
        let mut params = HashMap::new();
        params.insert(
            "creator".to_string(),
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        );
        params.insert(
            "receiver".to_string(),
            "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd".to_string(),
        );

        let resolvable = collect_resolvable_params(&params);
        assert_eq!(resolvable.is_empty(), true);
    }
}
