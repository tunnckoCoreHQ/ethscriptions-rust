// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use crate::cache::CacheTtl;
use crate::client::{Client, Failure};
use crate::digest::{compute_bytes_digest, estimate_gas};
use crate::encode::ungzip_bytes;
use crate::error::ApiError;
use crate::filters::normalize_filters;
use crate::normalize::{
    extract_numbers, extract_owners, normalize_ethscription, normalize_transfers,
};
use crate::types::{
    CheckExistResult, CostBreakdown, CostMeta, DigestResult, EstimateCostResult, Ethscription,
    EthscriptionListResult, EthscriptionTransfer, GasTrackerResponse, ModeResponse, PricesResult,
    RawEthscription, RawRecentPulseEvent, RecentPulseEvent, RecentPulseResult,
    UpstreamExistsResponse, UpstreamListResponse, UpstreamPulseResponse, UpstreamSingleResponse,
    UserProfileResult,
};
use crate::validate::{is_valid_address, is_valid_hash};
use url::form_urlencoded::Serializer;

/// Format an f64 as a clean string: no trailing zeros, no scientific notation.
///
/// Uses up to 12 significant decimal digits to avoid f64 binary representation noise
/// (e.g. `0.12` staying `"0.12"` instead of `"0.119999999999999996"`).
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        // Integer value — no decimal point needed
        format!("{v:.0}")
    } else {
        // Use 12 decimal digits (enough for ETH wei precision, avoids f64 noise)
        let s = format!("{v:.12}");
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        s.to_string()
    }
}

/// Error of the list endpoints (`/ethscriptions`, `/ethscription_transfers`).
fn list_error(failure: Failure) -> ApiError {
    match failure {
        Failure::Send(e) => ApiError::upstream_error(format!("Fetch failed unexpectedly: {e}")),
        Failure::Status(status) => {
            ApiError::upstream_error(format!("Upstream returned status {status}"))
        }
        Failure::Body(e) => ApiError::internal(format!("Failed to parse upstream response: {e}")),
    }
}

impl Client {
    /// Fetch a list of ethscriptions with optional filters.
    ///
    /// Corresponds to `GET /ethscriptions?{params}`.
    /// Returns `(normalized, raws, pagination)`.
    pub async fn fetch_ethscriptions(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<EthscriptionListResult, ApiError> {
        let base = &self.api_url;

        let mut params = params.clone();
        if params.remove("resolve").as_deref() == Some("true") {
            self.resolve_address_patches(&mut params).await;
        }

        let normalized = normalize_filters(&params);
        let query_string = build_qs(&normalized);
        let url = if query_string.is_empty() {
            format!("{base}/ethscriptions")
        } else {
            format!("{base}/ethscriptions?{query_string}")
        };

        let data: UpstreamListResponse = self.get_json(&url).await.map_err(list_error)?;

        let ethscriptions: Vec<Ethscription> =
            data.result.iter().map(normalize_ethscription).collect();

        Ok(EthscriptionListResult {
            pagination: data.pagination,
            raws: data.result,
            result: ethscriptions,
        })
    }

    /// Fetch blobscriptions (ethscriptions with attachments).
    ///
    /// Convenience wrapper around `fetch_ethscriptions` with `attachment_present=true`.
    pub async fn fetch_blobscriptions(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<EthscriptionListResult, ApiError> {
        let mut params = params.clone();
        params.insert("attachment_present".to_string(), "true".to_string());
        self.fetch_ethscriptions(&params).await
    }

    /// Fetch a single ethscription by ID.
    ///
    /// Corresponds to `GET /ethscriptions/{id}`.
    pub async fn fetch_ethscription(
        &self,
        id: &str,
    ) -> Result<(Ethscription, RawEthscription), ApiError> {
        let cache_key = cache_key_from_parts(&[&self.api_url, "ethscription", id]);
        if let Some(cached) = self
            .cache
            .load::<(Ethscription, RawEthscription)>("ethscription", &cache_key)?
        {
            return Ok(cached);
        }

        let result = self.fetch_ethscription_uncached(id).await?;
        self.cache
            .store("ethscription", &cache_key, CacheTtl::Week, &result)?;
        Ok(result)
    }

    async fn fetch_ethscription_uncached(
        &self,
        id: &str,
    ) -> Result<(Ethscription, RawEthscription), ApiError> {
        let url = format!("{}/ethscriptions/{id}", self.api_url);

        let data: UpstreamSingleResponse = self.get_json(&url).await.map_err(|f| match f {
            Failure::Status(_) => {
                ApiError::not_found("Transaction not found or it is not an Ethscription.")
            }
            f => list_error(f),
        })?;

        let normalized = normalize_ethscription(&data.result);
        Ok((normalized, data.result))
    }

    /// Fetch a single ethscription by ID and return specific mode details.
    ///
    /// Modes: "owners", "numbers", or `None` (default JSON).
    pub async fn fetch_ethscription_mode(
        &self,
        id: &str,
        mode: Option<&str>,
    ) -> Result<ModeResponse, ApiError> {
        match mode {
            Some("owners") => {
                let (eth, raw) = self.fetch_ethscription_uncached(id).await?;
                let transfers = extract_transfers(&raw);
                let owners = extract_owners(&eth, &raw, &transfers);
                Ok(ModeResponse::Json(
                    serde_json::to_value(&owners).unwrap_or_default(),
                ))
            }
            Some("numbers") => {
                let (eth, raw) = self.fetch_ethscription(id).await?;
                let transfers = extract_transfers(&raw);
                let numbers = extract_numbers(&eth, &raw, &transfers);
                Ok(ModeResponse::Json(
                    serde_json::to_value(&numbers).unwrap_or_default(),
                ))
            }
            Some("transfer") | Some("transfers") => {
                let (_, raw) = self.fetch_ethscription(id).await?;
                let transfers = extract_transfers(&raw);
                Ok(ModeResponse::Json(
                    serde_json::to_value(&transfers).unwrap_or_default(),
                ))
            }
            _ => {
                let (eth, _) = self.fetch_ethscription(id).await?;
                Ok(ModeResponse::Json(
                    serde_json::to_value(&eth).unwrap_or_default(),
                ))
            }
        }
    }

    /// Fetch ethscription content from a content URI.
    ///
    /// Handles both HTTP(S) URLs and `data:` URIs.
    /// Returns content data (raw bytes or base64-encoded) and the content type.
    ///
    /// # Arguments
    /// * `content_uri` - The content URI to fetch from
    /// * `base64` - If `Some(true)`, returns base64-encoded string; otherwise raw bytes
    pub async fn fetch_ethscription_content(
        &self,
        content_uri: &str,
        base64: Option<bool>,
        ungzip: Option<bool>,
    ) -> Result<(ContentData, String), ApiError> {
        let encode_base64 = base64.unwrap_or(false);
        let decode_gzip = ungzip.unwrap_or(false);

        // Handle data: URIs inline — reqwest can't fetch them
        if content_uri.starts_with("data:") {
            let (bytes, content_type) = parse_data_uri(content_uri)?;
            let bytes = if decode_gzip {
                ungzip_bytes(&bytes).unwrap_or(bytes)
            } else {
                bytes
            };
            let data = if encode_base64 {
                ContentData::Base64(bytes_to_base64(&bytes))
            } else {
                ContentData::Bytes(bytes)
            };
            return Ok((data, content_type));
        }

        let (content_type, bytes) = self.get_bytes(content_uri).await.map_err(|f| match f {
            Failure::Send(e) => ApiError::upstream_error(format!("Failed to fetch content: {e}")),
            Failure::Status(status) => ApiError::upstream_error(format!(
                "Failed to fetch content: upstream returned status {status}"
            )),
            Failure::Body(e) => ApiError::internal(format!("Failed to read content bytes: {e}")),
        })?;

        let bytes = if decode_gzip {
            ungzip_bytes(&bytes).unwrap_or_else(|_| bytes.to_vec())
        } else {
            bytes.to_vec()
        };

        let data = if encode_base64 {
            ContentData::Base64(bytes_to_base64(&bytes))
        } else {
            ContentData::Bytes(bytes)
        };

        Ok((data, content_type))
    }

    /// Fetch an ethscription's attachment (ESIP-8 blob).
    ///
    /// Corresponds to `GET /ethscriptions/{id}/attachment`.
    ///
    /// # Arguments
    /// * `id` - Ethscription ID (transaction hash or number)
    /// * `base64` - If `Some(true)`, returns base64-encoded string; otherwise raw bytes
    pub async fn fetch_ethscription_attachment(
        &self,
        id: &str,
        base64: Option<bool>,
        ungzip: Option<bool>,
    ) -> Result<ContentData, ApiError> {
        let encode_base64 = base64.unwrap_or(false);
        let decode_gzip = ungzip.unwrap_or(false);
        let url = format!("{}/ethscriptions/{id}/attachment", self.api_url);

        let (_, bytes) = self.get_bytes(&url).await.map_err(|f| match f {
            Failure::Send(e) => {
                ApiError::upstream_error(format!("Failed to fetch attachment: {e}"))
            }
            Failure::Status(_) => {
                ApiError::not_found("Attachment not found for this ethscription.")
            }
            Failure::Body(e) => ApiError::internal(format!("Failed to read attachment bytes: {e}")),
        })?;

        let bytes = if decode_gzip {
            ungzip_bytes(&bytes).unwrap_or_else(|_| bytes.to_vec())
        } else {
            bytes.to_vec()
        };

        let data = if encode_base64 {
            ContentData::Base64(bytes_to_base64(&bytes))
        } else {
            ContentData::Bytes(bytes)
        };

        Ok(data)
    }

    /// Check if an ethscription exists by SHA-256 hash.
    ///
    /// Corresponds to `GET /ethscriptions/exists/0x{sha}`.
    pub async fn check_exists(&self, sha: &str) -> Result<CheckExistResult, ApiError> {
        let base = &self.api_url;

        // Validate SHA
        let clean_sha = sha.strip_prefix("0x").unwrap_or(sha);
        if is_valid_hash(clean_sha) == false {
            return Err(ApiError::bad_request(
                "Invalid SHA-256 hash, must be 64 hex characters long, or 66 if 0x-prefixed",
            ));
        }

        let url = format!("{base}/ethscriptions/exists/0x{clean_sha}");
        let cache_key = cache_key_from_parts(&[base, "exists", clean_sha]);
        if let Some(cached) = self.cache.load::<CheckExistResult>("exists", &cache_key)? {
            return Ok(cached);
        }

        let data: UpstreamExistsResponse = self.get_json(&url).await.map_err(|f| match f {
            Failure::Send(e) => {
                ApiError::internal(format!("Failed to fetch from upstream API: {e}"))
            }
            Failure::Status(_) => {
                ApiError::bad_request("Failed checking if ethscription exists on the upstream API")
            }
            Failure::Body(e) => ApiError::internal(format!("Failed to parse exists response: {e}")),
        })?;

        if data.result.exists {
            let ethscription = data
                .result
                .ethscription
                .as_ref()
                .map(normalize_ethscription);
            let result = CheckExistResult {
                exists: true,
                ethscription,
            };
            self.cache
                .store("exists", &cache_key, CacheTtl::Week, &result)?;
            Ok(result)
        } else {
            Ok(CheckExistResult {
                exists: false,
                ethscription: None,
            })
        }
    }

    pub async fn fetch_user_created_ethscriptions(
        &self,
        value: &str,
        params: &HashMap<String, String>,
    ) -> Result<EthscriptionListResult, ApiError> {
        let mut params = params.clone();
        params.insert("creator".to_string(), value.to_string());
        if !is_valid_address(value) {
            params.insert("resolve".to_string(), "true".to_string());
        }
        self.fetch_ethscriptions(&params).await
    }

    pub async fn fetch_user_owned_ethscriptions(
        &self,
        value: &str,
        params: &HashMap<String, String>,
    ) -> Result<EthscriptionListResult, ApiError> {
        let mut params = params.clone();
        params.insert("current".to_string(), value.to_string());
        if !is_valid_address(value) {
            params.insert("resolve".to_string(), "true".to_string());
        }
        self.fetch_ethscriptions(&params).await
    }

    pub async fn fetch_user_profile(&self, value: &str) -> Result<UserProfileResult, ApiError> {
        let mut params = HashMap::new();
        params.insert("creator".to_string(), value.to_string());
        params.insert(
            "media_subtype".to_string(),
            "vnd.esc.user.profile+json".to_string(),
        );
        params.insert("media_type".to_string(), "application".to_string());

        if !is_valid_address(value) {
            params.insert("resolve".to_string(), "true".to_string());
        }

        let ethscriptions = self.fetch_ethscriptions(&params).await?;
        let Some((latest, previous)) = ethscriptions.result.split_first() else {
            return Err(ApiError::not_found(format!(
                "No user profile found for {value}"
            )));
        };

        Ok(UserProfileResult {
            latest: latest.clone(),
            previous: previous.to_vec(),
        })
    }

    pub async fn fetch_recent_pulse(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<RecentPulseResult, ApiError> {
        let base = &self.api_url;
        let mut params = params.clone();
        if params.remove("resolve").as_deref() == Some("true") {
            self.resolve_address_patches(&mut params).await;
        }

        let query_string = build_qs(&normalize_filters(&params));
        let url = if query_string.is_empty() {
            format!("{base}/ethscription_transfers")
        } else {
            format!("{base}/ethscription_transfers?{query_string}")
        };

        let data: UpstreamPulseResponse = self.get_json(&url).await.map_err(list_error)?;

        Ok(RecentPulseResult {
            pagination: data.pagination,
            result: data.result.iter().map(normalize_recent_pulse).collect(),
        })
    }

    pub async fn get_digest_for_data(
        &self,
        data: &[u8],
        check_existence: bool,
    ) -> Result<DigestResult, ApiError> {
        let (sha, hex, input) = compute_bytes_digest(data);
        let mut result = DigestResult {
            ethscription: None,
            exists: None,
            hex,
            input,
            sha: sha.clone(),
        };

        if check_existence {
            let exists = self.check_exists(&sha).await?;
            result.exists = Some(exists.exists);
            result.ethscription = exists.ethscription;
        }

        Ok(result)
    }

    pub async fn estimate_data_cost(
        &self,
        data: &[u8],
        speed: &str,
        buffer_fee: u64,
    ) -> Result<EstimateCostResult, ApiError> {
        let gas_needed = estimate_gas(data, buffer_fee);
        let prices = self.fetch_gas_prices(speed).await?;

        let gas_price_gwei: f64 = prices.gas_price.parse().map_err(|e| {
            ApiError::internal(format!(
                "Failed to parse gas_price '{}': {e}",
                prices.gas_price
            ))
        })?;
        let eth_price: f64 = prices.eth_price.parse().map_err(|e| {
            ApiError::internal(format!(
                "Failed to parse eth_price '{}': {e}",
                prices.eth_price
            ))
        })?;

        let total_gas_wei = gas_price_gwei * 1_000_000_000.0;
        let cost_wei = gas_needed as f64 * total_gas_wei;
        let cost_eth = cost_wei / 1_000_000_000_000_000_000.0;
        let cost_usd = cost_eth * eth_price;

        Ok(EstimateCostResult {
            prices,
            cost: CostBreakdown {
                wei: fmt_num(cost_wei),
                eth: fmt_num(cost_eth),
                usd: fmt_num(cost_usd),
            },
            meta: CostMeta {
                gas_needed: gas_needed.to_string(),
                input_length: data.len().to_string(),
            },
        })
    }

    /// Fetch gas prices from ethgastracker.com.
    pub async fn fetch_gas_prices(&self, speed: &str) -> Result<PricesResult, ApiError> {
        let data: GasTrackerResponse = self.get_json(&self.gas_url).await.map_err(|f| match f {
            Failure::Send(e) => ApiError::internal(format!("Failed to fetch gas prices: {e}")),
            Failure::Status(status) => {
                ApiError::upstream_error(format!("Failed to fetch gas prices: {status}"))
            }
            Failure::Body(e) => {
                ApiError::internal(format!("Failed to parse gas tracker response: {e}"))
            }
        })?;

        let oracle_speed = match speed {
            "fast" => &data.data.oracle.fast,
            _ => &data.data.oracle.normal,
        };

        Ok(PricesResult {
            block_number: data.data.block_nr,
            base_fee: fmt_num(data.data.base_fee),
            next_fee: fmt_num(data.data.next_fee),
            eth_price: fmt_num(data.data.eth_price),
            gas_price: fmt_num(oracle_speed.gwei),
            gas_fee: fmt_num(oracle_speed.gas_fee),
            priority_fee: fmt_num(oracle_speed.priority_fee),
        })
    }
}

/// Encode bytes as base64 string.
pub fn bytes_to_base64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Content data that can be either raw bytes or base64-encoded string.
#[derive(Debug, Clone)]
pub enum ContentData {
    /// Raw binary content
    Bytes(Vec<u8>),
    /// Base64-encoded content
    Base64(String),
}

impl ContentData {
    /// Get as bytes (decodes base64 if needed).
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            ContentData::Bytes(b) => b.clone(),
            ContentData::Base64(s) => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(s)
                    .unwrap_or_default()
            }
        }
    }

    /// Get as base64 string (encodes bytes if needed).
    pub fn as_base64(&self) -> String {
        match self {
            ContentData::Bytes(b) => bytes_to_base64(b),
            ContentData::Base64(s) => s.clone(),
        }
    }
}

/// Parse a `data:` URI into raw bytes and content type.
///
/// Supports `data:[<mediatype>][;base64],<data>` format.
fn parse_data_uri(uri: &str) -> Result<(Vec<u8>, String), ApiError> {
    let rest = uri
        .strip_prefix("data:")
        .ok_or_else(|| ApiError::internal("Invalid data URI"))?;

    let (meta, data) = rest
        .split_once(',')
        .ok_or_else(|| ApiError::internal("Invalid data URI: missing comma"))?;

    let is_base64 = meta.ends_with(";base64");
    let content_type = if is_base64 {
        meta.strip_suffix(";base64").unwrap_or(meta)
    } else {
        meta
    };
    let content_type = if content_type.is_empty() {
        "text/plain;charset=US-ASCII"
    } else {
        content_type
    };

    let bytes = if is_base64 {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| ApiError::internal(format!("Failed to decode base64 data URI: {e}")))?
    } else {
        percent_decode(data)?
    };

    Ok((bytes, content_type.to_string()))
}

fn percent_decode(input: &str) -> Result<Vec<u8>, ApiError> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(ApiError::internal(
                    "Invalid percent-encoding in data URI payload",
                ));
            }

            let hex = &input[index + 1..index + 3];
            let value = u8::from_str_radix(hex, 16)
                .map_err(|_| ApiError::internal("Invalid percent-encoding in data URI payload"))?;
            decoded.push(value);
            index += 3;
            continue;
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    Ok(decoded)
}

/// Fetch transfers for an ethscription from raw upstream data.
pub fn extract_transfers(raw: &RawEthscription) -> Vec<EthscriptionTransfer> {
    match &raw.ethscription_transfers {
        Some(transfers) => normalize_transfers(transfers),
        None => Vec::new(),
    }
}

/// Build a query string from a `HashMap`, sorted by key for determinism.
fn build_qs(params: &HashMap<String, String>) -> String {
    let mut pairs: Vec<(&str, &str)> = params
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    pairs.sort();

    let mut serializer = Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(key, value);
    }

    serializer.finish()
}

fn pulse_value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        _ => v.to_string(),
    }
}

fn pulse_timestamp_to_datetime(timestamp: &str) -> String {
    timestamp
        .parse::<i64>()
        .ok()
        .and_then(|ts| chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0))
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

fn normalize_recent_pulse(raw: &RawRecentPulseEvent) -> RecentPulseEvent {
    let block_timestamp = pulse_value_to_string(&raw.block_timestamp);
    let transfer_index = pulse_value_to_string(&raw.transfer_index);
    let event_type = if raw.ethscription_transaction_hash == raw.transaction_hash {
        "creation"
    } else {
        "transfer"
    };

    RecentPulseEvent {
        block_datetime: pulse_timestamp_to_datetime(&block_timestamp),
        block_hash: raw.block_blockhash.clone(),
        block_number: pulse_value_to_string(&raw.block_number),
        block_timestamp,
        event_type: event_type.to_string(),
        enforced_previous_owner: raw.enforced_previous_owner.clone(),
        ethscription_id: raw.ethscription_transaction_hash.clone(),
        event_log_index: raw.event_log_index.as_ref().and_then(|v| {
            if v.is_null() {
                None
            } else {
                Some(pulse_value_to_string(v))
            }
        }),
        from_address: raw.from_address.clone(),
        to_address: raw.to_address.clone(),
        transaction_hash: raw.transaction_hash.clone(),
        transaction_index: pulse_value_to_string(&raw.transaction_index),
        transfer_index,
    }
}

fn cache_key_from_parts(parts: &[&str]) -> String {
    compute_bytes_digest(parts.join("|").as_bytes()).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{RawEthscription, RawTransfer};

    // -----------------------------------------------------------------------
    // parse_data_uri
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_data_uri_plain_text() {
        let (bytes, ct) = parse_data_uri("data:,hello").unwrap();
        assert_eq!(bytes, b"hello");
        // empty mediatype defaults to text/plain;charset=US-ASCII
        assert_eq!(ct, "text/plain;charset=US-ASCII");
    }

    #[test]
    fn test_parse_data_uri_with_mediatype() {
        let (bytes, ct) = parse_data_uri("data:text/plain,hello world").unwrap();
        assert_eq!(bytes, b"hello world");
        assert_eq!(ct, "text/plain");
    }

    #[test]
    fn test_parse_data_uri_base64() {
        // "Hello" in base64 is "SGVsbG8="
        let (bytes, ct) = parse_data_uri("data:text/plain;base64,SGVsbG8=").unwrap();
        assert_eq!(bytes, b"Hello");
        assert_eq!(ct, "text/plain");
    }

    #[test]
    fn test_parse_data_uri_base64_no_mediatype() {
        // ;base64 with empty mediatype
        let (bytes, ct) = parse_data_uri("data:;base64,SGVsbG8=").unwrap();
        assert_eq!(bytes, b"Hello");
        assert_eq!(ct, "text/plain;charset=US-ASCII");
    }

    #[test]
    fn test_parse_data_uri_image_base64() {
        // Valid base64 for a few bytes (PNG magic bytes: 0x89 0x50 0x4E 0x47)
        let (bytes, ct) = parse_data_uri("data:image/png;base64,iVBORw==").unwrap();
        assert_eq!(ct, "image/png");
        assert_eq!(bytes.is_empty(), false);
        assert_eq!(bytes[0], 0x89); // PNG magic byte
    }

    #[test]
    fn test_parse_data_uri_json() {
        let (bytes, ct) = parse_data_uri("data:application/json,{\"key\":\"value\"}").unwrap();
        assert_eq!(ct, "application/json");
        assert_eq!(std::str::from_utf8(&bytes).unwrap(), "{\"key\":\"value\"}");
    }

    #[test]
    fn test_parse_data_uri_empty_data() {
        let (bytes, ct) = parse_data_uri("data:text/plain,").unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(ct, "text/plain");
    }

    #[test]
    fn test_parse_data_uri_with_charset_param() {
        let (bytes, ct) = parse_data_uri("data:text/plain;charset=utf-8,hello").unwrap();
        assert_eq!(bytes, b"hello");
        assert_eq!(ct, "text/plain;charset=utf-8");
    }

    #[test]
    fn test_parse_data_uri_percent_decodes_payload() {
        let (bytes, ct) = parse_data_uri("data:text/plain,hello%20world%21").unwrap();
        assert_eq!(bytes, b"hello world!");
        assert_eq!(ct, "text/plain");
    }

    #[test]
    fn test_parse_data_uri_missing_comma() {
        let result = parse_data_uri("data:text/plain");
        assert_eq!(result.is_err(), true);
    }

    #[test]
    fn test_parse_data_uri_no_prefix() {
        let result = parse_data_uri("http://example.com");
        assert_eq!(result.is_err(), true);
    }

    #[test]
    fn test_parse_data_uri_invalid_base64() {
        let result = parse_data_uri("data:text/plain;base64,!!!invalid!!!");
        assert_eq!(result.is_err(), true);
    }

    // -----------------------------------------------------------------------
    // extract_transfers
    // -----------------------------------------------------------------------

    fn make_raw_ethscription(transfers: Option<Vec<RawTransfer>>) -> RawEthscription {
        RawEthscription {
            transaction_hash: "0xabc".to_string(),
            block_number: Some(serde_json::json!(1)),
            block_blockhash: None,
            block_timestamp: Some(serde_json::json!(1000)),
            transaction_index: Some(serde_json::json!(0)),
            value: Some(serde_json::json!(0)),
            transaction_fee: Some(serde_json::json!(0)),
            gas_price: Some(serde_json::json!(0)),
            gas_used: Some(serde_json::json!(0)),
            event_log_index: Some(serde_json::Value::Null),
            creator: Some("0xcreator".to_string()),
            initial_owner: Some("0xreceiver".to_string()),
            current_owner: None,
            previous_owner: None,
            media_type: Some("text".to_string()),
            mime_subtype: Some("plain".to_string()),
            mimetype: Some("text/plain".to_string()),
            content_sha: None,
            content_uri: None,
            esip6: None,
            attachment_sha: None,
            attachment_content_type: None,
            ethscription_number: None,
            ethscription_transfers: transfers,
        }
    }

    #[test]
    fn test_extract_transfers_none() {
        let raw = make_raw_ethscription(None);
        let transfers = extract_transfers(&raw);
        assert_eq!(transfers.is_empty(), true);
    }

    #[test]
    fn test_extract_transfers_empty_vec() {
        let raw = make_raw_ethscription(Some(vec![]));
        let transfers = extract_transfers(&raw);
        assert_eq!(transfers.is_empty(), true);
    }

    #[test]
    fn test_extract_transfers_with_data() {
        let raw_transfers = vec![
            RawTransfer {
                ethscription_transaction_hash: Some("0xabc".to_string()),
                transaction_hash: Some("0xtx1".to_string()),
                from_address: Some("0xa".to_string()),
                to_address: Some("0xb".to_string()),
                block_number: Some(serde_json::json!(100)),
                block_timestamp: Some(serde_json::json!(1000)),
                block_blockhash: Some("0xblock1".to_string()),
                event_log_index: Some(serde_json::Value::Null),
                transaction_index: Some(serde_json::json!(0)),
                enforced_previous_owner: None,
            },
            RawTransfer {
                ethscription_transaction_hash: Some("0xabc".to_string()),
                transaction_hash: Some("0xtx2".to_string()),
                from_address: Some("0xb".to_string()),
                to_address: Some("0xc".to_string()),
                block_number: Some(serde_json::json!(200)),
                block_timestamp: Some(serde_json::json!(2000)),
                block_blockhash: Some("0xblock2".to_string()),
                event_log_index: Some(serde_json::json!(5)),
                transaction_index: Some(serde_json::json!(1)),
                enforced_previous_owner: None,
            },
        ];
        let raw = make_raw_ethscription(Some(raw_transfers));
        let transfers = extract_transfers(&raw);

        assert_eq!(transfers.len(), 2);
        // Sorted newest first (block 200 before 100)
        assert_eq!(transfers[0].block_number, "200");
        assert_eq!(transfers[0].from_address, "0xb");
        assert_eq!(transfers[0].to_address, "0xc");
        assert_eq!(transfers[0].event_log_index.as_deref(), Some("5"));
        assert_eq!(transfers[1].block_number, "100");
        assert_eq!(transfers[1].from_address, "0xa");
        assert_eq!(transfers[1].event_log_index, None);
    }

    #[test]
    fn test_extract_transfers_fields() {
        let raw_transfers = vec![RawTransfer {
            ethscription_transaction_hash: Some("0xabc".to_string()),
            transaction_hash: Some("0xtx1".to_string()),
            from_address: Some("0xa".to_string()),
            to_address: Some("0xb".to_string()),
            block_number: Some(serde_json::json!(100)),
            block_timestamp: Some(serde_json::json!(1000)),
            block_blockhash: None,
            event_log_index: Some(serde_json::Value::Null),
            transaction_index: Some(serde_json::json!(0)),
            enforced_previous_owner: None,
        }];
        let raw = make_raw_ethscription(Some(raw_transfers));
        let transfers = extract_transfers(&raw);

        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].block_number, "100");
    }

    // -----------------------------------------------------------------------
    // bytes_to_base64
    // -----------------------------------------------------------------------

    #[test]
    fn test_bytes_to_base64_hello() {
        let result = bytes_to_base64(b"Hello");
        assert_eq!(result, "SGVsbG8=");
    }

    #[test]
    fn test_bytes_to_base64_empty() {
        let result = bytes_to_base64(b"");
        assert_eq!(result, "");
    }

    #[test]
    fn test_bytes_to_base64_binary() {
        // PNG magic bytes
        let result = bytes_to_base64(&[0x89, 0x50, 0x4E, 0x47]);
        assert_eq!(result, "iVBORw==");
    }

    // -----------------------------------------------------------------------
    // ContentData
    // -----------------------------------------------------------------------

    #[test]
    fn test_content_data_bytes_as_bytes() {
        let data = ContentData::Bytes(vec![72, 101, 108, 108, 111]); // "Hello"
        assert_eq!(data.as_bytes(), b"Hello");
    }

    #[test]
    fn test_content_data_bytes_as_base64() {
        let data = ContentData::Bytes(b"Hello".to_vec());
        assert_eq!(data.as_base64(), "SGVsbG8=");
    }

    #[test]
    fn test_content_data_base64_as_base64() {
        let data = ContentData::Base64("SGVsbG8=".to_string());
        assert_eq!(data.as_base64(), "SGVsbG8=");
    }

    #[test]
    fn test_content_data_base64_as_bytes() {
        let data = ContentData::Base64("SGVsbG8=".to_string());
        assert_eq!(data.as_bytes(), b"Hello");
    }

    #[test]
    fn test_content_data_base64_invalid_returns_empty() {
        let data = ContentData::Base64("!!!invalid!!!".to_string());
        assert_eq!(data.as_bytes(), b"");
    }

    #[test]
    fn test_content_data_empty_bytes() {
        let data = ContentData::Bytes(vec![]);
        assert_eq!(data.as_bytes(), b"");
        assert_eq!(data.as_base64(), "");
    }

    // -----------------------------------------------------------------------
    // build_qs
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_qs_basic() {
        let mut params = HashMap::new();
        params.insert("creator".to_string(), "0xabc".to_string());
        params.insert("media_type".to_string(), "text".to_string());
        let qs = build_qs(&params);
        assert_eq!(qs, "creator=0xabc&media_type=text");
    }

    #[test]
    fn test_build_qs_sorted() {
        let mut params = HashMap::new();
        params.insert("z_param".to_string(), "last".to_string());
        params.insert("a_param".to_string(), "first".to_string());
        let qs = build_qs(&params);
        assert_eq!(qs, "a_param=first&z_param=last");
    }

    #[test]
    fn test_build_qs_filters_empty_values() {
        let mut params = HashMap::new();
        params.insert("creator".to_string(), "0xabc".to_string());
        params.insert("empty".to_string(), String::new());
        let qs = build_qs(&params);
        assert_eq!(qs, "creator=0xabc");
    }

    #[test]
    fn test_build_qs_empty_map() {
        let params = HashMap::new();
        let qs = build_qs(&params);
        assert_eq!(qs, "");
    }

    #[test]
    fn test_build_qs_single_param() {
        let mut params = HashMap::new();
        params.insert("per_page".to_string(), "25".to_string());
        let qs = build_qs(&params);
        assert_eq!(qs, "per_page=25");
    }

    #[test]
    fn test_build_qs_url_encodes_values() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), "wgw test".to_string());
        params.insert(
            "content_type".to_string(),
            "text/plain;charset=utf-8".to_string(),
        );
        let qs = build_qs(&params);
        assert_eq!(
            qs,
            "content_type=text%2Fplain%3Bcharset%3Dutf-8&name=wgw+test"
        );
    }
}
