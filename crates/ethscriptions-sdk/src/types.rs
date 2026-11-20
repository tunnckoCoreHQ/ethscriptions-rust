// SPDX-License-Identifier: MPL-2.0

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ---------------------------------------------------------------------------
// Core Ethscription types
// ---------------------------------------------------------------------------

/// Normalized ethscription metadata.
///
/// Field names match the SDK conventions (not the upstream API names).
/// The normalizer handles the rename from upstream fields like `initial_owner` -> `receiver`,
/// `mime_subtype` -> `media_subtype`, `mimetype` -> `content_type`, etc.
///
/// Note: `attachment_*` fields are stripped when null (only present for ESIP-8).
/// `content_uri` and `event_log_index` are stripped by default (available via `--with`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Ethscription {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_sha: Option<String>,
    pub block_datetime: String,
    pub block_hash: String,
    pub block_number: String,
    pub block_timestamp: String,
    pub content_path: String,
    pub content_sha: String,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_uri: Option<String>,
    pub creator: String,
    pub ethscription_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_log_index: Option<String>,
    pub gas_price: String,
    pub gas_used: String,
    pub has_transfers: bool,
    pub is_esip3: bool,
    pub is_esip4: bool,
    pub is_esip6: bool,
    pub is_esip8: bool,
    pub media_subtype: String,
    pub media_type: String,
    pub receiver: String,
    pub transaction_fee: String,
    pub transaction_hash: String,
    pub transaction_index: String,
    pub transaction_value: String,
}

/// Raw upstream ethscription as returned by the V2 API (before normalization).
///
/// Uses `serde(default)` and `Option` liberally because upstream responses vary.
/// Fields are sorted alphabetically for determinism.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawEthscription {
    #[serde(default)]
    pub attachment_content_type: Option<String>,
    #[serde(default)]
    pub attachment_sha: Option<String>,
    #[serde(default)]
    pub block_blockhash: Option<String>,
    #[serde(default)]
    pub block_number: Option<serde_json::Value>,
    #[serde(default)]
    pub block_timestamp: Option<serde_json::Value>,
    #[serde(default)]
    pub content_sha: Option<String>,
    #[serde(default)]
    pub content_uri: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub current_owner: Option<String>,
    #[serde(default)]
    pub esip6: Option<bool>,
    #[serde(default)]
    pub ethscription_number: Option<serde_json::Value>,
    #[serde(default)]
    pub ethscription_transfers: Option<Vec<RawTransfer>>,
    #[serde(default)]
    pub event_log_index: Option<serde_json::Value>,
    #[serde(default)]
    pub gas_price: Option<serde_json::Value>,
    #[serde(default)]
    pub gas_used: Option<serde_json::Value>,
    #[serde(default)]
    pub initial_owner: Option<String>,
    #[serde(default)]
    pub media_type: Option<String>,
    #[serde(default)]
    pub mime_subtype: Option<String>,
    #[serde(default)]
    pub mimetype: Option<String>,
    #[serde(default)]
    pub previous_owner: Option<String>,
    #[serde(default)]
    pub transaction_fee: Option<serde_json::Value>,
    pub transaction_hash: String,
    #[serde(default)]
    pub transaction_index: Option<serde_json::Value>,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
}

/// Raw upstream transfer as returned by the V2 API.
/// Fields are sorted alphabetically for determinism.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTransfer {
    #[serde(default)]
    pub block_blockhash: Option<String>,
    #[serde(default)]
    pub block_number: Option<serde_json::Value>,
    #[serde(default)]
    pub block_timestamp: Option<serde_json::Value>,
    #[serde(default)]
    pub enforced_previous_owner: Option<serde_json::Value>,
    #[serde(default)]
    pub ethscription_transaction_hash: Option<String>,
    #[serde(default)]
    pub event_log_index: Option<serde_json::Value>,
    #[serde(default)]
    pub from_address: Option<String>,
    #[serde(default)]
    pub to_address: Option<String>,
    #[serde(default)]
    pub transaction_hash: Option<String>,
    #[serde(default)]
    pub transaction_index: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Normalized transfer
// ---------------------------------------------------------------------------

/// Normalized ethscription transfer event.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EthscriptionTransfer {
    pub block_hash: String,
    pub block_number: String,
    pub block_timestamp: String,
    pub block_datetime: String,
    pub enforced_previous_owner: Option<String>,
    pub ethscription_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_log_index: Option<String>,
    pub from_address: String,
    pub is_contract_transfer: bool,
    pub to_address: String,
    pub transaction_hash: String,
    pub transaction_index: String,
}

// ---------------------------------------------------------------------------
// Result types returned by public SDK functions
// ---------------------------------------------------------------------------

/// Response for mode-based fetching (JSON or raw binary).
pub enum ModeResponse {
    Json(serde_json::Value),
    Binary(Vec<u8>, String),
}

/// Result from `check_exists`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CheckExistResult {
    pub ethscription: Option<Ethscription>,
    pub exists: bool,
}

/// Result from `resolve_user`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResolveUserResult {
    pub address: String,
    pub name: String,
}

/// Result from `get_user_profile`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserProfileResult {
    pub latest: Ethscription,
    pub previous: Vec<Ethscription>,
}

/// Result from `get_digest_for_data`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DigestResult {
    pub ethscription: Option<Ethscription>,
    pub exists: Option<bool>,
    pub hex: String,
    pub input: String,
    pub sha: String,
}

/// Normalized recent pulse event as returned by `/ethscription_transfers`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RecentPulseEvent {
    pub block_datetime: String,
    pub block_hash: String,
    pub block_number: String,
    pub block_timestamp: String,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforced_previous_owner: Option<String>,
    pub ethscription_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_log_index: Option<String>,
    pub from_address: String,
    pub to_address: String,
    pub transaction_hash: String,
    pub transaction_index: String,
    pub transfer_index: String,
}

/// Raw recent pulse event as returned by upstream `/ethscription_transfers`.
#[derive(Debug, Clone, Deserialize)]
pub struct RawRecentPulseEvent {
    pub block_blockhash: String,
    pub block_number: serde_json::Value,
    pub block_timestamp: serde_json::Value,
    #[serde(default)]
    pub enforced_previous_owner: Option<String>,
    pub ethscription_transaction_hash: String,
    #[serde(default)]
    pub event_log_index: Option<serde_json::Value>,
    pub from_address: String,
    pub to_address: String,
    pub transaction_hash: String,
    pub transaction_index: serde_json::Value,
    pub transfer_index: serde_json::Value,
}

/// Ownership chain result.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OwnersResult {
    pub creator: String,
    pub current: String,
    pub initial: String,
    pub latest_transfer_block: String,
    pub latest_transfer_datetime: String,
    pub latest_transfer_hash: String,
    pub latest_transfer_timestamp: String,
    pub previous: String,
}

/// Numbering / statistics result.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NumbersResult {
    pub block_datetime: String,
    pub block_hash: String,
    pub block_number: String,
    pub block_number_fmt: String,
    pub block_timestamp: String,
    pub ethscription_number: Option<String>,
    pub ethscription_number_fmt: String,
    pub ethscription_transfers: String,
    pub event_log_index: Option<String>,
    pub transaction_index: String,
}

/// Gas prices fetched from ethgastracker.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PricesResult {
    pub base_fee: String,
    pub block_number: String,
    pub eth_price: String,
    pub gas_fee: String,
    pub gas_price: String,
    pub next_fee: String,
    pub priority_fee: String,
}

/// Cost estimation result.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EstimateCostResult {
    pub cost: CostBreakdown,
    pub meta: CostMeta,
    pub prices: PricesResult,
}

/// Cost breakdown in wei / eth / usd.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CostBreakdown {
    pub eth: String,
    pub usd: String,
    pub wei: String,
}

/// Metadata about the cost estimation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CostMeta {
    pub gas_needed: String,
    pub input_length: String,
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

/// Pagination info returned from upstream list endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Pagination {
    pub has_more: bool,
    pub page_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthscriptionListResult {
    pub pagination: Option<Pagination>,
    pub raws: Vec<RawEthscription>,
    pub result: Vec<Ethscription>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentPulseResult {
    pub pagination: Option<Pagination>,
    pub result: Vec<RecentPulseEvent>,
}

// ---------------------------------------------------------------------------
// Generic API response envelope
// ---------------------------------------------------------------------------

/// Wrapper for successful API responses with optional pagination and headers.
#[derive(Debug, Clone, Serialize)]
pub struct ApiOk<T: Serialize> {
    pub ok: bool,
    pub pagination: Option<Pagination>,
    pub result: T,
}

impl<T: Serialize> ApiOk<T> {
    pub fn new(result: T) -> Self {
        Self {
            ok: true,
            result,
            pagination: None,
        }
    }

    pub fn with_pagination(mut self, pagination: Option<Pagination>) -> Self {
        self.pagination = pagination;
        self
    }
}

/// Error shape returned to clients (matches the TS `NotOkShape`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErr {
    pub ok: bool,
    pub error: ApiErrDetail,
}

/// Inner error detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrDetail {
    pub http_status: u16,
    pub message: String,
}

impl ApiErr {
    pub fn new(http_status: u16, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: ApiErrDetail {
                message: message.into(),
                http_status,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Upstream API response shapes (for deserialization)
// ---------------------------------------------------------------------------

/// Upstream single-item response: `{ result: {...} }`
#[derive(Debug, Clone, Deserialize)]
pub struct UpstreamSingleResponse {
    pub result: RawEthscription,
}

/// Upstream list response: `{ result: [...], pagination: {...} }`
#[derive(Debug, Clone, Deserialize)]
pub struct UpstreamListResponse {
    pub result: Vec<RawEthscription>,
    #[serde(default)]
    pub pagination: Option<Pagination>,
}

/// Upstream exists check response: `{ result: { exists: bool, ethscription?: {...} } }`
#[derive(Debug, Clone, Deserialize)]
pub struct UpstreamExistsResponse {
    pub result: UpstreamExistsResult,
}

/// Inner exists result.
#[derive(Debug, Clone, Deserialize)]
pub struct UpstreamExistsResult {
    pub exists: bool,
    #[serde(default)]
    pub ethscription: Option<RawEthscription>,
}

/// Upstream recent pulse response: `{ result: [...], pagination: {...} }`
#[derive(Debug, Clone, Deserialize)]
pub struct UpstreamPulseResponse {
    pub result: Vec<RawRecentPulseEvent>,
    #[serde(default)]
    pub pagination: Option<Pagination>,
}

// ---------------------------------------------------------------------------
// Ethgas tracker response shapes
// ---------------------------------------------------------------------------

/// Response from ethgastracker.com/api/gas/latest
#[derive(Debug, Clone, Deserialize)]
pub struct GasTrackerResponse {
    pub data: GasTrackerData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GasTrackerData {
    #[serde(rename = "blockNr")]
    pub block_nr: String,
    #[serde(rename = "baseFee")]
    pub base_fee: f64,
    #[serde(rename = "nextFee")]
    pub next_fee: f64,
    #[serde(rename = "ethPrice")]
    pub eth_price: f64,
    pub oracle: GasOracle,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GasOracle {
    pub slow: GasSpeed,
    pub normal: GasSpeed,
    pub fast: GasSpeed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GasSpeed {
    #[serde(rename = "gasFee")]
    pub gas_fee: f64,
    #[serde(rename = "priorityFee")]
    pub priority_fee: f64,
    pub gwei: f64,
}

// ---------------------------------------------------------------------------
// ENS data response shape
// ---------------------------------------------------------------------------

/// Response from api.ensdata.net
#[derive(Debug, Clone, Deserialize)]
pub struct EnsDataResponse {
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub ens: Option<String>,
    #[serde(default)]
    pub ens_primary: Option<String>,
}

// ---------------------------------------------------------------------------
// Filter struct (for query parameter mapping)
// ---------------------------------------------------------------------------

/// SDK-friendly filter parameters for listing ethscriptions.
///
/// These use the normalized SDK field names;
/// `normalize_filters` converts them to the upstream API parameter names.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EthscriptionFilters {
    // -- Address filters --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// Alias for `initial_owner` upstream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>,
    /// Alias for `current_owner` upstream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Alias for `current_owner` upstream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    /// Alias for `initial_owner` upstream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial: Option<String>,
    /// Alias for `previous_owner` upstream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,

    // -- Content type filters --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// Alias for `mime_subtype` upstream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_subtype: Option<String>,
    /// Alias for `mimetype` upstream (can be multiple)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<Vec<String>>,

    // -- Content / hash filters --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ethscription_number: Option<String>,

    // -- Block filters --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_blockhash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_block: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_block: Option<u64>,

    // -- Attachment / ESIP-8 filters --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_present: Option<bool>,

    // -- Token filters --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_tick: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_protocol: Option<String>,

    // -- Transfer filter --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transferred_in_tx: Option<String>,

    // -- Response shape --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_hash_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_latest_transfer: Option<bool>,

    // -- Sorting --
    /// "newest_first" (default) or "oldest_first"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverse: Option<bool>,

    // -- Pagination --
    /// Alias for `max_results` upstream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<u32>,
    /// Alias for `max_results` upstream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u32>,
    /// Alias for `page_key` upstream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_key: Option<String>,

    // -- SDK-only (not sent upstream) --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve: Option<bool>,
    /// Field selection: extra fields to include
    #[serde(rename = "with", skip_serializing_if = "Option::is_none")]
    pub with_fields: Option<String>,
    /// Field selection: only include these fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<String>,
}
