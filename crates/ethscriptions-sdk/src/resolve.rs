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
        let lowercased = value.to_lowercase();

        if let Some(ens_data) = self.ens_api_handler(&lowercased).await
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

    async fn ens_api_handler(&self, val: &str) -> Option<EnsDataResponse> {
        let url = format!("{}/{val}", self.ens_url);
        self.get_json(&url).await.ok()
    }

    /// Replace a name in an address filter with its resolved address.
    ///
    /// Leaves empty values, valid addresses, and names that do not resolve unchanged.
    pub async fn resolve_address_field(&self, field: &mut Option<String>) {
        let Some(value) = field.as_deref() else {
            return;
        };
        if value.is_empty() || is_valid_address(value) {
            return;
        }
        if let Some(resolved) = self.names_resolver(value, false, false, false).await {
            *field = Some(resolved);
        }
    }
}
