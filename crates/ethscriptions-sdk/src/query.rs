// SPDX-License-Identifier: MPL-2.0

use url::form_urlencoded::Serializer;

use crate::client::Client;

/// Filters for `GET /ethscriptions`.
#[derive(Debug, Clone, Default)]
pub struct ListQuery {
    pub creator: Option<String>,
    /// Upstream `initial_owner`.
    pub receiver: Option<String>,
    pub current_owner: Option<String>,
    pub previous_owner: Option<String>,
    pub media_type: Option<String>,
    /// Upstream `mime_subtype`.
    pub media_subtype: Option<String>,
    /// Upstream `mimetype`.
    pub content_type: Option<String>,
    pub content_sha: Option<String>,
    pub block_number: Option<String>,
    /// Upstream `block_blockhash`.
    pub block_hash: Option<String>,
    pub after_block: Option<u64>,
    pub before_block: Option<u64>,
    pub attachment_sha: Option<String>,
    pub attachment_content_type: Option<String>,
    pub attachment_present: bool,
    pub token_tick: Option<String>,
    pub token_protocol: Option<String>,
    pub transferred_in_tx: Option<String>,
    pub transaction_hash_only: bool,
    pub sort_by: Option<String>,
    pub reverse: bool,
    /// Upstream `max_results`.
    pub page_size: Option<u32>,
    pub page_key: Option<String>,
    /// Resolve names in the address filters to addresses before the request.
    pub resolve: bool,
}

impl ListQuery {
    /// Replace names in the address filters with resolved addresses, if `resolve` is set.
    pub async fn resolve_names(&mut self, client: &Client) {
        if !self.resolve {
            return;
        }
        for field in [
            &mut self.creator,
            &mut self.receiver,
            &mut self.current_owner,
            &mut self.previous_owner,
        ] {
            client.resolve_address_field(field).await;
        }
    }

    /// Upstream query string: upstream names, sorted by name, URL-encoded, empty values left out.
    pub fn to_query_string(&self) -> String {
        encode_pairs(vec![
            ("creator", self.creator.clone()),
            ("initial_owner", self.receiver.clone()),
            ("current_owner", self.current_owner.clone()),
            ("previous_owner", self.previous_owner.clone()),
            ("media_type", self.media_type.clone()),
            ("mime_subtype", self.media_subtype.clone()),
            ("mimetype", self.content_type.clone()),
            ("content_sha", self.content_sha.clone()),
            ("block_number", self.block_number.clone()),
            ("block_blockhash", self.block_hash.clone()),
            ("after_block", self.after_block.map(|n| n.to_string())),
            ("before_block", self.before_block.map(|n| n.to_string())),
            ("attachment_sha", self.attachment_sha.clone()),
            (
                "attachment_content_type",
                self.attachment_content_type.clone(),
            ),
            ("attachment_present", flag(self.attachment_present)),
            ("token_tick", self.token_tick.clone()),
            ("token_protocol", self.token_protocol.clone()),
            ("transferred_in_tx", self.transferred_in_tx.clone()),
            ("transaction_hash_only", flag(self.transaction_hash_only)),
            ("sort_by", self.sort_by.clone()),
            ("reverse", flag(self.reverse)),
            ("max_results", self.page_size.map(|n| n.to_string())),
            ("page_key", self.page_key.clone()),
        ])
    }
}

/// Filters for `GET /ethscription_transfers`, the recent pulse feed.
#[derive(Debug, Clone, Default)]
pub struct PulseQuery {
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    pub to_or_from: Option<String>,
    pub transaction_hash: Option<String>,
    /// Upstream `ethscription_token_tick`.
    pub token_tick: Option<String>,
    /// Upstream `ethscription_token_protocol`.
    pub token_protocol: Option<String>,
    pub sort_by: Option<String>,
    pub reverse: bool,
    /// Upstream `max_results`.
    pub page_size: Option<u32>,
    pub page_key: Option<String>,
    /// Resolve names in the address filters to addresses before the request.
    pub resolve: bool,
}

impl PulseQuery {
    /// Replace names in the address filters with resolved addresses, if `resolve` is set.
    pub async fn resolve_names(&mut self, client: &Client) {
        if !self.resolve {
            return;
        }
        for field in [
            &mut self.from_address,
            &mut self.to_address,
            &mut self.to_or_from,
        ] {
            client.resolve_address_field(field).await;
        }
    }

    /// Upstream query string: upstream names, sorted by name, URL-encoded, empty values left out.
    pub fn to_query_string(&self) -> String {
        encode_pairs(vec![
            ("from_address", self.from_address.clone()),
            ("to_address", self.to_address.clone()),
            ("to_or_from", self.to_or_from.clone()),
            ("transaction_hash", self.transaction_hash.clone()),
            ("ethscription_token_tick", self.token_tick.clone()),
            ("ethscription_token_protocol", self.token_protocol.clone()),
            ("sort_by", self.sort_by.clone()),
            ("reverse", flag(self.reverse)),
            ("max_results", self.page_size.map(|n| n.to_string())),
            ("page_key", self.page_key.clone()),
        ])
    }
}

fn flag(on: bool) -> Option<String> {
    on.then(|| "true".to_string())
}

fn encode_pairs(mut pairs: Vec<(&str, Option<String>)>) -> String {
    pairs.sort_by_key(|(key, _)| *key);
    let mut serializer = Serializer::new(String::new());
    for (key, value) in pairs {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            serializer.append_pair(key, &value);
        }
    }
    serializer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_query_empty() {
        assert_eq!(ListQuery::default().to_query_string(), "");
    }

    #[test]
    fn test_list_query_maps_sdk_names_to_upstream_names() {
        let query = ListQuery {
            receiver: Some("0xr".to_string()),
            media_subtype: Some("plain".to_string()),
            content_type: Some("text".to_string()),
            block_hash: Some("0xb".to_string()),
            page_size: Some(25),
            ..ListQuery::default()
        };
        assert_eq!(
            query.to_query_string(),
            "block_blockhash=0xb&initial_owner=0xr&max_results=25&mime_subtype=plain&mimetype=text"
        );
    }

    #[test]
    fn test_list_query_flags_and_empty_values() {
        let query = ListQuery {
            creator: Some(String::new()),
            attachment_present: true,
            reverse: true,
            resolve: true,
            ..ListQuery::default()
        };
        assert_eq!(
            query.to_query_string(),
            "attachment_present=true&reverse=true"
        );
    }

    #[test]
    fn test_list_query_url_encodes_values() {
        let query = ListQuery {
            content_type: Some("text/plain;charset=utf-8".to_string()),
            page_key: Some("wgw test".to_string()),
            ..ListQuery::default()
        };
        assert_eq!(
            query.to_query_string(),
            "mimetype=text%2Fplain%3Bcharset%3Dutf-8&page_key=wgw+test"
        );
    }

    #[test]
    fn test_pulse_query_maps_sdk_names_to_upstream_names() {
        let query = PulseQuery {
            token_tick: Some("eths".to_string()),
            token_protocol: Some("erc-20".to_string()),
            page_size: Some(5),
            ..PulseQuery::default()
        };
        assert_eq!(
            query.to_query_string(),
            "ethscription_token_protocol=erc-20&ethscription_token_tick=eths&max_results=5"
        );
    }
}
