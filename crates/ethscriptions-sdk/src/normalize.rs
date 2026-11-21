// SPDX-License-Identifier: MPL-2.0

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

use crate::types::{
    Ethscription, EthscriptionTransfer, FieldSelection, NumbersResult, OwnersResult,
    RawEthscription, RawTransfer, Record,
};

// ---------------------------------------------------------------------------
// JSON key sorting for deterministic output
// ---------------------------------------------------------------------------

/// Recursively sort all keys in a JSON value for deterministic output.
///
/// This ensures that JSON serialization produces the same output regardless
/// of the order fields were added to objects. Uses `BTreeMap` internally
/// which maintains alphabetical key order.
pub fn sort_json_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<String, Value> = map
                .into_iter()
                .map(|(k, v)| (k, sort_json_keys(v)))
                .collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(sort_json_keys).collect()),
        other => other,
    }
}

/// Convert a serializable value to a JSON Value with sorted keys.
///
/// This is a convenience function that combines serialization with key sorting.
pub fn to_sorted_json<T: Serialize>(value: &T) -> Value {
    let json_value = serde_json::to_value(value).expect("Failed to serialize to JSON value");
    sort_json_keys(json_value)
}

/// Convert a serializable value to a JSON string with sorted keys.
///
/// Use `pretty = true` for formatted output with indentation.
pub fn to_sorted_json_string<T: Serialize>(value: &T, pretty: bool) -> String {
    let sorted = to_sorted_json(value);
    if pretty {
        serde_json::to_string_pretty(&sorted).expect("Failed to serialize to JSON")
    } else {
        serde_json::to_string(&sorted).expect("Failed to serialize to JSON")
    }
}

// ---------------------------------------------------------------------------
// String utilities
// ---------------------------------------------------------------------------

/// Strip trailing `.0` from a numeric string (e.g. `"1234.0"` -> `"1234"`).
fn strip_trailing_dot_zero(s: &str) -> String {
    s.strip_suffix(".0").unwrap_or(s).to_string()
}

/// Convert a value (which may be a number or string) to a `String`.
fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Convert a unix timestamp (seconds) to an ISO 8601 datetime string.
fn timestamp_to_datetime(timestamp_str: &str) -> String {
    let ts: i64 = timestamp_str.parse().unwrap_or(0);
    let dt = DateTime::<Utc>::from_timestamp(ts, 0);
    match dt {
        Some(d) => d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        None => String::new(),
    }
}

/// Normalize a raw upstream ethscription into the SDK `Ethscription` shape.
///
/// Handles field renaming:
/// - `initial_owner` -> `receiver`
/// - `mime_subtype` -> `media_subtype`
/// - `mimetype` -> `content_type`
/// - `value` -> `transaction_value`
///
/// Also computes:
/// - `block_datetime` from `block_timestamp`
/// - `content_path` as `/ethscriptions/{tx_hash}/content`
/// - ESIP boolean flags
/// - Strips `.0` suffixes from numeric strings
pub fn normalize_ethscription(raw: &RawEthscription) -> Ethscription {
    let tx_hash = &raw.transaction_hash;
    let block_timestamp = raw
        .block_timestamp
        .as_ref()
        .map(value_to_string)
        .unwrap_or_default();

    let event_log_index = raw.event_log_index.as_ref().and_then(|v| {
        if v.is_null() {
            None
        } else {
            Some(value_to_string(v))
        }
    });

    let content_uri = raw.content_uri.clone();

    // ESIP flags
    let is_esip3 = event_log_index.is_some();
    let is_esip4 = content_uri
        .as_deref()
        .is_some_and(|uri| uri.contains("vnd.facet.tx+json"));
    let is_esip6 = raw.esip6.unwrap_or(false);
    let is_esip8 = raw.attachment_sha.is_some();

    let has_transfers = raw
        .ethscription_transfers
        .as_ref()
        .map(|transfers| {
            transfers.iter().any(|t| {
                let eth_id = t
                    .ethscription_transaction_hash
                    .as_deref()
                    .unwrap_or_default();
                let tx_hash = t.transaction_hash.as_deref().unwrap_or_default();
                eth_id != tx_hash && !eth_id.is_empty()
            })
        })
        .unwrap_or(false);

    // Attachment fields (only for ESIP-8)
    let (attachment_content_type, attachment_media_type, attachment_path, attachment_sha) =
        if let Some(ref att_sha) = raw.attachment_sha {
            let att_ct = raw.attachment_content_type.clone().unwrap_or_default();
            let att_mt = att_ct.split('/').next().map(String::from);
            let att_path = format!("/ethscriptions/{tx_hash}/attachment");
            (Some(att_ct), att_mt, Some(att_path), Some(att_sha.clone()))
        } else {
            (None, None, None, None)
        };

    // Fields are ordered alphabetically for deterministic JSON serialization
    Ethscription {
        attachment_content_type,
        attachment_media_type,
        attachment_path,
        attachment_sha,
        block_datetime: timestamp_to_datetime(&block_timestamp),
        block_hash: raw.block_blockhash.clone().unwrap_or_default(),
        block_number: raw
            .block_number
            .as_ref()
            .map(value_to_string)
            .unwrap_or_default(),
        block_timestamp: block_timestamp.clone(),
        content_path: format!("/ethscriptions/{tx_hash}/content"),
        content_sha: raw.content_sha.clone().unwrap_or_default(),
        content_type: raw.mimetype.clone().unwrap_or_default(),
        content_uri: None,
        creator: raw.creator.clone().unwrap_or_default(),
        ethscription_id: tx_hash.clone(),
        event_log_index,
        gas_price: raw
            .gas_price
            .as_ref()
            .map(|v| strip_trailing_dot_zero(&value_to_string(v)))
            .unwrap_or_default(),
        gas_used: raw
            .gas_used
            .as_ref()
            .map(value_to_string)
            .unwrap_or_default(),
        has_transfers,
        is_esip3,
        is_esip4,
        is_esip6,
        is_esip8,
        media_subtype: raw.mime_subtype.clone().unwrap_or_default(),
        media_type: raw.media_type.clone().unwrap_or_default(),
        receiver: raw.initial_owner.clone().unwrap_or_default(),
        transaction_fee: raw
            .transaction_fee
            .as_ref()
            .map(|v| strip_trailing_dot_zero(&value_to_string(v)))
            .unwrap_or_default(),
        transaction_hash: tx_hash.clone(),
        transaction_index: raw
            .transaction_index
            .as_ref()
            .map(value_to_string)
            .unwrap_or_default(),
        transaction_value: raw
            .value
            .as_ref()
            .map(|v| strip_trailing_dot_zero(&value_to_string(v)))
            .unwrap_or_default(),
    }
}

/// Normalize and sort transfers from raw upstream data.
///
/// - Removes `ethscription_transaction_hash` (redundant)
/// - Sorts by block_number, transaction_index, event_log_index descending (newest first)
pub fn normalize_transfers(raw_transfers: &[RawTransfer]) -> Vec<EthscriptionTransfer> {
    let mut transfers: Vec<EthscriptionTransfer> = raw_transfers
        .iter()
        .filter(|raw| {
            let eth_id = raw
                .ethscription_transaction_hash
                .as_deref()
                .unwrap_or_default();
            let tx_hash = raw.transaction_hash.as_deref().unwrap_or_default();
            eth_id.is_empty() || eth_id != tx_hash
        })
        .map(|raw| {
            let block_timestamp = raw
                .block_timestamp
                .as_ref()
                .map(value_to_string)
                .unwrap_or_default();

            let enforced_previous_owner = raw.enforced_previous_owner.as_ref().and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    Some(value_to_string(v))
                }
            });

            EthscriptionTransfer {
                block_hash: raw.block_blockhash.clone().unwrap_or_default(),
                block_number: raw
                    .block_number
                    .as_ref()
                    .map(value_to_string)
                    .unwrap_or_default(),
                block_datetime: timestamp_to_datetime(&block_timestamp),
                block_timestamp,
                is_contract_transfer: enforced_previous_owner.is_some(),
                enforced_previous_owner,
                ethscription_id: raw
                    .ethscription_transaction_hash
                    .clone()
                    .unwrap_or_default(),
                event_log_index: raw.event_log_index.as_ref().and_then(|v| {
                    if v.is_null() {
                        None
                    } else {
                        Some(value_to_string(v))
                    }
                }),
                from_address: raw.from_address.clone().unwrap_or_default(),
                to_address: raw.to_address.clone().unwrap_or_default(),
                transaction_hash: raw.transaction_hash.clone().unwrap_or_default(),
                transaction_index: raw
                    .transaction_index
                    .as_ref()
                    .map(value_to_string)
                    .unwrap_or_default(),
            }
        })
        .collect();

    // Sort by block_number, transaction_index, event_log_index descending (newest first)
    transfers.sort_by(|a, b| {
        let a_block: u64 = a.block_number.parse().unwrap_or(0);
        let b_block: u64 = b.block_number.parse().unwrap_or(0);

        match b_block.cmp(&a_block) {
            std::cmp::Ordering::Equal => {
                let a_tx_idx: u64 = a.transaction_index.parse().unwrap_or(0);
                let b_tx_idx: u64 = b.transaction_index.parse().unwrap_or(0);

                match b_tx_idx.cmp(&a_tx_idx) {
                    std::cmp::Ordering::Equal => {
                        let a_evt: u64 = a
                            .event_log_index
                            .as_deref()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        let b_evt: u64 = b
                            .event_log_index
                            .as_deref()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        b_evt.cmp(&a_evt)
                    }
                    other => other,
                }
            }
            other => other,
        }
    });

    transfers
}

impl From<RawEthscription> for Record {
    fn from(raw: RawEthscription) -> Self {
        Self {
            normalized: normalize_ethscription(&raw),
            raw,
        }
    }
}

impl Record {
    /// Normalized transfers, newest first.
    pub fn transfers(&self) -> Vec<EthscriptionTransfer> {
        match &self.raw.ethscription_transfers {
            Some(transfers) => normalize_transfers(transfers),
            None => Vec::new(),
        }
    }

    pub fn owners(&self) -> OwnersResult {
        let ethscription = &self.normalized;
        let transfers = self.transfers();
        let (latest_ts, latest_block, latest_hash) = if let Some(t) = transfers.first() {
            (
                t.block_timestamp.clone(),
                t.block_number.clone(),
                t.transaction_hash.clone(),
            )
        } else {
            (
                ethscription.block_timestamp.clone(),
                ethscription.block_number.clone(),
                ethscription.transaction_hash.clone(),
            )
        };

        OwnersResult {
            creator: ethscription.creator.clone(),
            current: self.raw.current_owner.clone().unwrap_or_default(),
            initial: ethscription.receiver.clone(),
            latest_transfer_block: latest_block,
            latest_transfer_datetime: timestamp_to_datetime(&latest_ts),
            latest_transfer_hash: latest_hash,
            latest_transfer_timestamp: latest_ts.clone(),
            previous: self.raw.previous_owner.clone().unwrap_or_default(),
        }
    }

    pub fn numbers(&self) -> NumbersResult {
        let ethscription = &self.normalized;
        let eth_num_str = self
            .raw
            .ethscription_number
            .as_ref()
            .map(value_to_string)
            .unwrap_or_default();

        let eth_num = if eth_num_str.is_empty() {
            None
        } else {
            Some(eth_num_str.clone())
        };

        NumbersResult {
            block_datetime: ethscription.block_datetime.clone(),
            block_hash: ethscription.block_hash.clone(),
            block_number: ethscription.block_number.clone(),
            block_number_fmt: numfmt(&ethscription.block_number, ","),
            block_timestamp: ethscription.block_timestamp.clone(),
            ethscription_number: eth_num.clone(),
            ethscription_number_fmt: numfmt(&eth_num.unwrap_or_default(), ","),
            ethscription_transfers: self.transfers().len().to_string(),
            event_log_index: ethscription.event_log_index.clone(),
            transaction_index: ethscription.transaction_index.clone(),
        }
    }

    /// Apply field selection to the normalized ethscription.
    ///
    /// - `only`: keep only these fields from the normalized output
    /// - `with`: add extra fields not in the default normalized output,
    ///   pulling from the raw upstream data first, then from the normalized result
    ///
    /// Special handling:
    /// - `ethscription_transfers`: normalized through `normalize_transfers()` when requested
    /// - `transfers`: alias for `ethscription_transfers` but output as `transfers`
    ///
    /// The `latest_transfer` flag adds `latest_transfer_hash` and `latest_transfer_block` fields
    /// computed from the most recent transfer.
    pub fn select(&self, selection: &FieldSelection) -> Value {
        let withs = &selection.with;
        let onlys = &selection.only;
        let wants = |field: &str| withs.iter().any(|w| w == field);

        let norm_value = serde_json::to_value(&self.normalized).unwrap_or(Value::Null);
        let raw_value = serde_json::to_value(&self.raw).unwrap_or(Value::Null);

        let norm_obj = match &norm_value {
            Value::Object(m) => m,
            _ => return norm_value,
        };
        let raw_obj = match &raw_value {
            Value::Object(m) => m,
            _ => &serde_json::Map::new(),
        };

        let mut result = serde_json::Map::new();
        for (key, val) in norm_obj {
            if key == "event_log_index" && !wants("event_log_index") {
                continue;
            }
            if onlys.is_empty() || onlys.contains(key) {
                result.insert(key.clone(), val.clone());
            }
        }

        let needs_transfers =
            selection.latest_transfer || wants("ethscription_transfers") || wants("transfers");

        let normalized_transfers = if needs_transfers {
            self.raw
                .ethscription_transfers
                .as_ref()
                .map(|t| normalize_transfers(t))
        } else {
            None
        };

        if selection.latest_transfer
            && let Some(ref transfers) = normalized_transfers
            && let Some(latest) = transfers.first()
        {
            result.insert(
                "latest_transfer_hash".to_string(),
                Value::String(latest.transaction_hash.clone()),
            );
            result.insert(
                "latest_transfer_block".to_string(),
                Value::String(latest.block_number.clone()),
            );
        }

        for key in withs {
            if key == "ethscription_transfers" || key == "transfers" {
                if !result.contains_key(key)
                    && let Some(ref transfers) = normalized_transfers
                    && let Ok(transfers_json) = serde_json::to_value(transfers)
                {
                    result.insert(key.clone(), transfers_json);
                }
                continue;
            }

            if key == "event_log_index" && !result.contains_key(key) {
                if let Some(v) = raw_obj.get(key)
                    && !v.is_null()
                {
                    result.insert(key.clone(), v.clone());
                }
                continue;
            }

            if !result.contains_key(key) {
                if let Some(v) = raw_obj.get(key) {
                    result.insert(key.clone(), v.clone());
                } else if let Some(v) = norm_obj.get(key) {
                    result.insert(key.clone(), v.clone());
                }
            }
        }

        Value::Object(result)
    }
}

impl FieldSelection {
    /// Parse the comma-separated `with` and `only` lists. Each item is trimmed.
    pub fn new(with: Option<&str>, only: Option<&str>, latest_transfer: bool) -> Self {
        Self {
            with: split_fields(with),
            only: split_fields(only),
            latest_transfer,
        }
    }

    /// Add `field` to `with` unless it is already there.
    pub fn with_field(mut self, field: &str) -> Self {
        if !self.with.iter().any(|w| w == field) {
            self.with.push(field.to_string());
        }
        self
    }
}

fn split_fields(fields: Option<&str>) -> Vec<String> {
    fields
        .filter(|s| !s.is_empty())
        .map(|s| s.split(',').map(|f| f.trim().to_string()).collect())
        .unwrap_or_default()
}

/// Format a numeric string with delimiter (e.g. `"1234567"` -> `"1,234,567"`).
pub fn numfmt(x: &str, delim: &str) -> String {
    if x.is_empty() {
        return String::new();
    }

    let chars: Vec<char> = x.chars().collect();
    let mut result = Vec::new();
    for (i, ch) in chars.iter().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(delim.to_string());
        }
        result.push(ch.to_string());
    }
    result.reverse();
    result.join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_json_keys_simple_object() {
        let input = serde_json::json!({
            "zebra": 1,
            "apple": 2,
            "mango": 3
        });
        let sorted = sort_json_keys(input);
        let keys: Vec<&String> = sorted.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn test_sort_json_keys_nested_objects() {
        let input = serde_json::json!({
            "outer_z": {
                "inner_b": 1,
                "inner_a": 2
            },
            "outer_a": {
                "inner_z": 3,
                "inner_m": 4
            }
        });
        let sorted = sort_json_keys(input);

        // Check outer keys are sorted
        let outer_keys: Vec<&String> = sorted.as_object().unwrap().keys().collect();
        assert_eq!(outer_keys, vec!["outer_a", "outer_z"]);

        // Check inner keys are sorted
        let inner_a_keys: Vec<&String> = sorted["outer_a"].as_object().unwrap().keys().collect();
        assert_eq!(inner_a_keys, vec!["inner_m", "inner_z"]);
    }

    #[test]
    fn test_sort_json_keys_array_of_objects() {
        let input = serde_json::json!([
            {"z": 1, "a": 2},
            {"y": 3, "b": 4}
        ]);
        let sorted = sort_json_keys(input);

        let first_keys: Vec<&String> = sorted[0].as_object().unwrap().keys().collect();
        let second_keys: Vec<&String> = sorted[1].as_object().unwrap().keys().collect();
        assert_eq!(first_keys, vec!["a", "z"]);
        assert_eq!(second_keys, vec!["b", "y"]);
    }

    #[test]
    fn test_to_sorted_json_string() {
        #[derive(Serialize)]
        struct TestStruct {
            zebra: i32,
            apple: i32,
        }
        let value = TestStruct { zebra: 1, apple: 2 };
        let json_str = to_sorted_json_string(&value, false);
        assert_eq!(json_str, r#"{"apple":2,"zebra":1}"#);
    }

    #[test]
    fn test_strip_trailing_dot_zero() {
        assert_eq!(strip_trailing_dot_zero("1234.0"), "1234");
        assert_eq!(strip_trailing_dot_zero("1234"), "1234");
        assert_eq!(strip_trailing_dot_zero("0.0"), "0");
        assert_eq!(strip_trailing_dot_zero("3.14"), "3.14");
    }

    #[test]
    fn test_timestamp_to_datetime() {
        // 1700000000 = 2023-11-14T22:13:20Z
        let dt = timestamp_to_datetime("1700000000");
        assert_eq!(dt.starts_with("2023-11-14T22:13:20"), true);
    }

    #[test]
    fn test_numfmt() {
        assert_eq!(numfmt("1234567", ","), "1,234,567");
        assert_eq!(numfmt("123", ","), "123");
        assert_eq!(numfmt("1234", "."), "1.234");
        assert_eq!(numfmt("", ","), "");
    }

    #[test]
    fn test_normalize_ethscription_field_renames() {
        let raw = RawEthscription {
            attachment_content_type: None,
            attachment_sha: None,
            block_blockhash: Some("0xblockhash".to_string()),
            block_number: Some(serde_json::json!(12345)),
            block_timestamp: Some(serde_json::json!(1700000000)),
            content_sha: Some("sha256hash".to_string()),
            content_uri: None,
            creator: Some("0xcreator".to_string()),
            current_owner: Some("0xcurrent".to_string()),
            esip6: Some(false),
            ethscription_number: None,
            ethscription_transfers: None,
            event_log_index: Some(serde_json::Value::Null),
            gas_price: Some(serde_json::json!("20.0")),
            gas_used: Some(serde_json::json!(21000)),
            initial_owner: Some("0xreceiver".to_string()),
            media_type: Some("text".to_string()),
            mime_subtype: Some("plain".to_string()),
            mimetype: Some("text/plain".to_string()),
            previous_owner: Some("0xprevious".to_string()),
            transaction_fee: Some(serde_json::json!("50.0")),
            transaction_hash: "0xabc123".to_string(),
            transaction_index: Some(serde_json::json!(5)),
            value: Some(serde_json::json!("100.0")),
        };

        let normalized = normalize_ethscription(&raw);

        // Field renames
        assert_eq!(normalized.receiver, "0xreceiver"); // initial_owner -> receiver
        assert_eq!(normalized.media_subtype, "plain"); // mime_subtype -> media_subtype
        assert_eq!(normalized.content_type, "text/plain"); // mimetype -> content_type
        assert_eq!(normalized.transaction_value, "100"); // value -> transaction_value, stripped .0

        // Computed fields
        assert_eq!(normalized.block_datetime.starts_with("2023-11-14"), true);
        assert_eq!(normalized.content_path, "/ethscriptions/0xabc123/content");

        // .0 stripping
        assert_eq!(normalized.transaction_fee, "50");
        assert_eq!(normalized.gas_price, "20");

        assert_eq!(normalized.has_transfers, false);
        assert_eq!(normalized.is_esip3, false);
        assert_eq!(normalized.is_esip8, false);
    }

    #[test]
    fn test_normalize_ethscription_with_attachment() {
        let raw = RawEthscription {
            attachment_content_type: Some("image/png".to_string()),
            attachment_sha: Some("attsha".to_string()),
            block_blockhash: None,
            block_number: Some(serde_json::json!(1)),
            block_timestamp: Some(serde_json::json!(0)),
            content_sha: None,
            content_uri: None,
            creator: None,
            current_owner: None,
            esip6: None,
            ethscription_number: None,
            ethscription_transfers: None,
            event_log_index: Some(serde_json::json!(3)),
            gas_price: Some(serde_json::json!(0)),
            gas_used: Some(serde_json::json!(0)),
            initial_owner: None,
            media_type: None,
            mime_subtype: None,
            mimetype: None,
            previous_owner: None,
            transaction_fee: Some(serde_json::json!(0)),
            transaction_hash: "0xabc".to_string(),
            transaction_index: Some(serde_json::json!(0)),
            value: Some(serde_json::json!(0)),
        };

        let normalized = normalize_ethscription(&raw);
        assert_eq!(normalized.is_esip8, true);
        assert_eq!(normalized.has_transfers, false); // no transfers
        assert_eq!(normalized.is_esip3, true);
        assert_eq!(
            normalized.attachment_content_type,
            Some("image/png".to_string())
        );
        assert_eq!(normalized.attachment_media_type, Some("image".to_string()));
        assert_eq!(
            normalized.attachment_path,
            Some("/ethscriptions/0xabc/attachment".to_string())
        );
    }

    #[test]
    fn test_normalize_transfers_sorts_and_flags() {
        let raw = vec![
            RawTransfer {
                block_blockhash: None,
                block_number: Some(serde_json::json!(100)),
                block_timestamp: Some(serde_json::json!(1000)),
                enforced_previous_owner: None,
                ethscription_transaction_hash: Some("0xtx3".to_string()),
                event_log_index: Some(serde_json::Value::Null),
                from_address: Some("0xa".to_string()),
                to_address: Some("0xb".to_string()),
                transaction_hash: Some("0xtx1".to_string()),
                transaction_index: Some(serde_json::json!(0)),
            },
            RawTransfer {
                block_blockhash: None,
                block_number: Some(serde_json::json!(200)),
                block_timestamp: Some(serde_json::json!(2000)),
                enforced_previous_owner: Some(serde_json::json!("0xcontract")),
                ethscription_transaction_hash: Some("0xtx4".to_string()),
                event_log_index: Some(serde_json::json!(5)),
                from_address: Some("0xb".to_string()),
                to_address: Some("0xc".to_string()),
                transaction_hash: Some("0xtx2".to_string()),
                transaction_index: Some(serde_json::json!(1)),
            },
        ];

        let transfers = normalize_transfers(&raw);

        // Sorted newest first (block 200 before block 100)
        assert_eq!(transfers[0].block_number, "200");
        assert_eq!(transfers[0].ethscription_id, "0xtx4");
        assert_eq!(transfers[0].is_contract_transfer, true);

        assert_eq!(transfers[1].block_number, "100");
        assert_eq!(transfers[1].ethscription_id, "0xtx3");
        assert_eq!(transfers[1].is_contract_transfer, false);
    }

    fn select(
        raw: RawEthscription,
        with: Option<&str>,
        only: Option<&str>,
        latest_transfer: Option<bool>,
    ) -> Value {
        Record::from(raw).select(&FieldSelection::new(
            with,
            only,
            latest_transfer.unwrap_or(false),
        ))
    }

    fn make_test_raw() -> RawEthscription {
        RawEthscription {
            attachment_content_type: None,
            attachment_sha: None,
            block_blockhash: Some("0xblockhash".to_string()),
            block_number: Some(serde_json::json!(12345)),
            block_timestamp: Some(serde_json::json!(1700000000)),
            content_sha: Some("sha256hash".to_string()),
            content_uri: Some("data:,hello".to_string()),
            creator: Some("0xcreator".to_string()),
            current_owner: Some("0xcurrent".to_string()),
            esip6: Some(false),
            ethscription_number: Some(serde_json::json!(42)),
            ethscription_transfers: None,
            event_log_index: Some(serde_json::Value::Null),
            gas_price: Some(serde_json::json!("20.0")),
            gas_used: Some(serde_json::json!(21000)),
            initial_owner: Some("0xreceiver".to_string()),
            media_type: Some("text".to_string()),
            mime_subtype: Some("plain".to_string()),
            mimetype: Some("text/plain".to_string()),
            previous_owner: Some("0xprevious".to_string()),
            transaction_fee: Some(serde_json::json!("50.0")),
            transaction_hash: "0xabc123".to_string(),
            transaction_index: Some(serde_json::json!(5)),
            value: Some(serde_json::json!("100.0")),
        }
    }

    #[test]
    fn test_select_no_filters() {
        let raw = make_test_raw();
        let result = select(raw, None, None, None);
        let obj = result.as_object().unwrap();

        // Should have all default fields
        assert_eq!(obj.contains_key("transaction_hash"), true);
        assert_eq!(obj.contains_key("creator"), true);
        assert_eq!(obj.contains_key("receiver"), true);
        assert_eq!(obj.contains_key("has_transfers"), true);

        // content_uri and event_log_index are skipped by default (skip_serializing_if = None)
        assert_eq!(obj.contains_key("content_uri"), false);
        assert_eq!(obj.contains_key("event_log_index"), false);

        // attachment_* fields are also skipped when None
        assert_eq!(obj.contains_key("attachment_sha"), false);
    }

    #[test]
    fn test_select_only() {
        let raw = make_test_raw();
        let result = select(raw, None, Some("transaction_hash,creator"), None);
        let obj = result.as_object().unwrap();

        assert_eq!(obj.len(), 2);
        assert_eq!(obj["transaction_hash"], "0xabc123");
        assert_eq!(obj["creator"], "0xcreator");
    }

    #[test]
    fn test_select_with_from_raw() {
        let raw = make_test_raw();
        // current_owner is not in normalized at all, but exists in raw
        // content_uri is skipped in normalized, so `with` pulls it from raw
        let result = select(raw, Some("current_owner,content_uri"), None, None);
        let obj = result.as_object().unwrap();

        // Should include extra field pulled from raw
        assert_eq!(obj["current_owner"], "0xcurrent");

        // content_uri is now pulled from raw when requested via --with
        assert_eq!(obj["content_uri"], "data:,hello");

        // Should still have all normalized fields
        assert_eq!(obj.contains_key("transaction_hash"), true);
        assert_eq!(obj.contains_key("creator"), true);
    }

    #[test]
    fn test_select_with_and_only_combined() {
        let raw = make_test_raw();
        let result = select(
            raw,
            Some("ethscription_number,current_owner"),
            Some("transaction_hash,creator"),
            None,
        );
        let obj = result.as_object().unwrap();

        // only=transaction_hash,creator limits to those 2 fields
        // with=ethscription_number,current_owner adds 2 more from raw
        assert_eq!(obj.len(), 4);
        assert_eq!(obj["transaction_hash"], "0xabc123");
        assert_eq!(obj["creator"], "0xcreator");
        assert_eq!(obj["ethscription_number"], 42);
        assert_eq!(obj["current_owner"], "0xcurrent");
    }

    #[test]
    fn test_select_with_existing_field_no_duplicate() {
        let raw = make_test_raw();
        // "creator" is already in the normalized output — `with` should not duplicate it
        let result = select(raw, Some("creator"), None, None);
        let obj = result.as_object().unwrap();

        // creator should appear exactly once
        assert_eq!(obj["creator"], "0xcreator");
    }

    fn make_test_raw_with_transfers() -> RawEthscription {
        let mut raw = make_test_raw();
        raw.ethscription_transfers = Some(vec![
            RawTransfer {
                block_blockhash: Some("0xblockhash1".to_string()),
                block_number: Some(serde_json::json!(100)),
                block_timestamp: Some(serde_json::json!(1000)),
                enforced_previous_owner: None,
                ethscription_transaction_hash: Some("0xabc123".to_string()),
                event_log_index: Some(serde_json::Value::Null),
                from_address: Some("0xcreator".to_string()),
                to_address: Some("0xreceiver".to_string()),
                transaction_hash: Some("0xabc123".to_string()),
                transaction_index: Some(serde_json::json!(0)),
            },
            RawTransfer {
                block_blockhash: Some("0xblockhash2".to_string()),
                block_number: Some(serde_json::json!(200)),
                block_timestamp: Some(serde_json::json!(2000)),
                enforced_previous_owner: None,
                ethscription_transaction_hash: Some("0xabc123".to_string()),
                event_log_index: Some(serde_json::json!(5)),
                from_address: Some("0xreceiver".to_string()),
                to_address: Some("0xcurrent".to_string()),
                transaction_hash: Some("0xtransfer1".to_string()),
                transaction_index: Some(serde_json::json!(1)),
            },
        ]);
        raw
    }

    #[test]
    fn test_select_with_ethscription_transfers() {
        let raw = make_test_raw_with_transfers();
        let result = select(raw, Some("ethscription_transfers"), None, None);
        let obj = result.as_object().unwrap();

        // Should include normalized transfers
        assert_eq!(obj.contains_key("ethscription_transfers"), true);

        let transfers = obj["ethscription_transfers"].as_array().unwrap();
        assert_eq!(transfers.len(), 1);

        // Transfers should be sorted newest first (block 200 before block 100)
        assert_eq!(transfers[0]["block_number"], "200");

        // Should have normalized field names
        assert_eq!(transfers[0].get("ethscription_id").is_some(), true);
        assert_eq!(transfers[0].get("block_datetime").is_some(), true);
    }

    #[test]
    fn test_select_with_transfers_alias() {
        let raw = make_test_raw_with_transfers();
        let result = select(raw, Some("transfers"), None, None);
        let obj = result.as_object().unwrap();

        // Should use "transfers" as the output key (not "ethscription_transfers")
        assert_eq!(obj.contains_key("transfers"), true);
        assert_eq!(obj.contains_key("ethscription_transfers"), false);

        let transfers = obj["transfers"].as_array().unwrap();
        assert_eq!(transfers.len(), 1);

        // Transfers should be sorted and normalized
        assert_eq!(transfers[0]["block_number"], "200");
    }

    #[test]
    fn test_select_transfers_no_data() {
        let raw = make_test_raw(); // No transfers
        let result = select(raw, Some("transfers"), None, None);
        let obj = result.as_object().unwrap();

        // Should not include transfers key when there's no data
        assert_eq!(obj.contains_key("transfers"), false);
    }

    #[test]
    fn test_select_with_latest_transfer_flag() {
        let raw = make_test_raw_with_transfers();
        let result = select(raw, None, None, Some(true));
        let obj = result.as_object().unwrap();

        // Should include latest_transfer_hash and latest_transfer_block from the newest transfer (block 200)
        assert_eq!(obj.contains_key("latest_transfer_hash"), true);
        assert_eq!(obj["latest_transfer_hash"], "0xtransfer1");
        assert_eq!(obj.contains_key("latest_transfer_block"), true);
        assert_eq!(obj["latest_transfer_block"], "200");
    }

    #[test]
    fn test_select_latest_transfer_no_transfers() {
        let raw = make_test_raw(); // No transfers
        let result = select(raw, None, None, Some(true));
        let obj = result.as_object().unwrap();

        // Should not include latest_transfer fields when there's no data
        assert_eq!(obj.contains_key("latest_transfer_hash"), false);
        assert_eq!(obj.contains_key("latest_transfer_block"), false);
    }
}
