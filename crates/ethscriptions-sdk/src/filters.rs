// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use url::form_urlencoded::Serializer;

/// Normalize SDK-friendly filter parameters to upstream API parameter names.
///
/// Mapping:
/// - `media_subtype` -> `mime_subtype`
/// - `per_page` / `page_size` -> `max_results`
/// - `cursor` -> `page_key`
/// - `receiver` / `initial` -> `initial_owner`
/// - `previous` -> `previous_owner`
/// - `current` / `owner` -> `current_owner`
/// - `content_type` -> `mimetype`
/// - `block_hash` -> `block_blockhash`
pub fn normalize_filters(params: &HashMap<String, String>) -> HashMap<String, String> {
    let mut out = HashMap::new();

    for (key, value) in params {
        match key.as_str() {
            "media_subtype" => {
                out.insert("mime_subtype".to_string(), value.clone());
            }
            "per_page" | "page_size" => {
                out.insert("max_results".to_string(), value.clone());
            }
            "cursor" => {
                out.insert("page_key".to_string(), value.clone());
            }
            "receiver" | "initial" => {
                out.insert("initial_owner".to_string(), value.clone());
            }
            "previous" => {
                out.insert("previous_owner".to_string(), value.clone());
            }
            "current" | "owner" => {
                out.insert("current_owner".to_string(), value.clone());
            }
            "content_type" => {
                out.insert("mimetype".to_string(), value.clone());
            }
            "block_hash" => {
                out.insert("block_blockhash".to_string(), value.clone());
            }
            // Pass through anything that doesn't need remapping
            _ => {
                out.insert(key.clone(), value.clone());
            }
        }
    }

    // Remove SDK-only keys that shouldn't be sent upstream
    out.remove("resolve");
    out.remove("baseURL");
    out.remove("with");
    out.remove("only");

    out
}

/// Build an upstream query string from normalized filter parameters.
///
/// Joins key-value pairs with `&`, no URL-encoding (upstream API doesn't need it
/// for the values we send). Handles multi-value params like `mimetype[]`.
pub fn build_query_string(params: &HashMap<String, String>) -> String {
    let normalized = normalize_filters(params);
    let mut pairs: Vec<(&str, &str)> = normalized
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    pairs.sort();

    let mut serializer = Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(key, value);
    }

    serializer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_media_subtype() {
        let mut params = HashMap::new();
        params.insert("media_subtype".to_string(), "plain".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("mime_subtype").unwrap(), "plain");
        assert_eq!(result.contains_key("media_subtype"), false);
    }

    #[test]
    fn test_normalize_per_page() {
        let mut params = HashMap::new();
        params.insert("per_page".to_string(), "25".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("max_results").unwrap(), "25");
    }

    #[test]
    fn test_normalize_page_size() {
        let mut params = HashMap::new();
        params.insert("page_size".to_string(), "50".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("max_results").unwrap(), "50");
    }

    #[test]
    fn test_normalize_cursor() {
        let mut params = HashMap::new();
        params.insert("cursor".to_string(), "0xabc".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("page_key").unwrap(), "0xabc");
        assert_eq!(result.contains_key("cursor"), false);
    }

    #[test]
    fn test_normalize_receiver() {
        let mut params = HashMap::new();
        params.insert("receiver".to_string(), "0xaddr".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("initial_owner").unwrap(), "0xaddr");
        assert_eq!(result.contains_key("receiver"), false);
    }

    #[test]
    fn test_normalize_owner_aliases() {
        let mut params = HashMap::new();
        params.insert("owner".to_string(), "0xabc123".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("current_owner").unwrap(), "0xabc123");

        let mut params = HashMap::new();
        params.insert("current".to_string(), "0xcurrent".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("current_owner").unwrap(), "0xcurrent");

        let mut params = HashMap::new();
        params.insert("previous".to_string(), "0xprev".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("previous_owner").unwrap(), "0xprev");

        let mut params = HashMap::new();
        params.insert("initial".to_string(), "0xinit".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("initial_owner").unwrap(), "0xinit");
    }

    #[test]
    fn test_normalize_content_type() {
        let mut params = HashMap::new();
        params.insert("content_type".to_string(), "text/plain".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("mimetype").unwrap(), "text/plain");
        assert_eq!(result.contains_key("content_type"), false);
    }

    #[test]
    fn test_normalize_removes_sdk_only_keys() {
        let mut params = HashMap::new();
        params.insert("resolve".to_string(), "true".to_string());
        params.insert("baseURL".to_string(), "http://example.com".to_string());
        params.insert("creator".to_string(), "0xabc".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.contains_key("resolve"), false);
        assert_eq!(result.contains_key("baseURL"), false);
        assert_eq!(result.get("creator").unwrap(), "0xabc");
    }

    #[test]
    fn test_passthrough_unknown_keys() {
        let mut params = HashMap::new();
        params.insert("creator".to_string(), "0xabc".to_string());
        params.insert("media_type".to_string(), "text".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("creator").unwrap(), "0xabc");
        assert_eq!(result.get("media_type").unwrap(), "text");
    }

    #[test]
    fn test_normalize_block_hash() {
        let mut params = HashMap::new();
        params.insert("block_hash".to_string(), "0xabc123".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("block_blockhash").unwrap(), "0xabc123");
        assert_eq!(result.contains_key("block_hash"), false);
    }

    #[test]
    fn test_normalize_page_key_passthrough() {
        let mut params = HashMap::new();
        params.insert("page_key".to_string(), "abc123".to_string());
        let result = normalize_filters(&params);
        assert_eq!(result.get("page_key").unwrap(), "abc123");
    }

    #[test]
    fn test_build_query_string() {
        let mut params = HashMap::new();
        params.insert("creator".to_string(), "0xabc".to_string());
        params.insert("media_subtype".to_string(), "plain".to_string());
        let qs = build_query_string(&params);
        assert_eq!(qs.contains("creator=0xabc"), true);
        assert_eq!(qs.contains("mime_subtype=plain"), true);
        assert_eq!(qs.contains("media_subtype"), false);
    }
}
