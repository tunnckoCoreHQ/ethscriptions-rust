// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use alloy::eips::eip4844::BlobTransactionSidecar;
use alloy::eips::eip4844::builder::{SidecarBuilder, SimpleCoder};
use alloy::network::{EthereumWallet, TransactionBuilder, TransactionBuilder4844};
use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy_signer_local::{MnemonicBuilder, PrivateKeySigner, coins_bip39::English};
use serde::Serialize;

use crate::client::Client;
use crate::digest::compute_digest;
use crate::encode::{
    DataUriOptions, InputSource, build_data_uri_with_type, detect_content_type, gzip_bytes,
    optimize_image,
};
use crate::error::ApiError;
use crate::types::EstimateCostResult;

#[derive(Debug, Clone, Serialize)]
pub struct PrepareInscribeOptions {
    pub source: InputSource,
    pub optimize: bool,
    pub gzip: bool,
    pub esip6: bool,
    pub skip_content_type: bool,
    pub data_uri_limit: usize,
}

impl Default for PrepareInscribeOptions {
    fn default() -> Self {
        Self {
            source: InputSource::Literal,
            optimize: false,
            gzip: false,
            esip6: false,
            skip_content_type: false,
            data_uri_limit: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PreparedInscribe {
    pub content_type: String,
    pub data_uri: String,
    pub data_uri_truncated: String,
    pub gzipped_size: Option<usize>,
    pub sha: String,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DryRunInscribeResult {
    pub chain_id: u64,
    pub content_type: String,
    pub data_uri: String,
    pub exists: bool,
    pub from: String,
    pub gzipped_size: Option<usize>,
    pub mode: String,
    pub sha: String,
    pub size: usize,
    pub to: String,
    pub estimate: Option<EstimateCostResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubmittedInscribeResult {
    pub chain_id: u64,
    pub content_type: String,
    pub from: String,
    pub gzipped_size: Option<usize>,
    pub size: usize,
    pub to: String,
    pub transaction_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum InscribeResult {
    DryRun(Box<DryRunInscribeResult>),
    Submitted(SubmittedInscribeResult),
}

#[derive(Debug, Clone)]
pub struct InscribeOptions {
    pub bytes: Vec<u8>,
    pub source: InputSource,
    pub optimize: bool,
    pub gzip: bool,
    pub esip6: bool,
    pub skip_content_type: bool,
    pub blobs: bool,
    pub dry_run: bool,
    pub private_key: Option<String>,
    pub mnemonic: Option<String>,
    pub env_file: Option<String>,
    pub rpc_url: Option<String>,
    pub to: Option<String>,
    pub chain_id: u64,
}

pub async fn prepare_inscribe(
    client: &Client,
    bytes: &[u8],
    options: &PrepareInscribeOptions,
) -> Result<(PreparedInscribe, EstimateCostResult), ApiError> {
    let mut content_type = detect_content_type(bytes);
    let mut transformed = bytes.to_vec();
    let original_size = transformed.len();

    if options.optimize
        && let Some(optimized) = optimize_image(&transformed, &content_type)?
    {
        transformed = optimized;
        content_type = "image/webp".to_string();
    }

    let gzipped_size = if options.gzip && options.source != InputSource::Literal {
        transformed = gzip_bytes(&transformed)?;
        Some(transformed.len())
    } else {
        None
    };

    let data_uri = build_data_uri_with_type(
        &transformed,
        &content_type,
        &DataUriOptions {
            skip_content_type: options.skip_content_type,
            esip6: options.esip6,
        },
    );
    let sha = compute_digest(data_uri.as_bytes());
    let estimate = client
        .estimate_data_cost(data_uri.as_bytes(), "fast", 0)
        .await?;

    Ok((
        PreparedInscribe {
            content_type,
            data_uri_truncated: truncate_data_uri(&data_uri, options.data_uri_limit),
            data_uri,
            gzipped_size,
            sha,
            size: original_size,
        },
        estimate,
    ))
}

pub async fn inscribe(
    client: &Client,
    options: &InscribeOptions,
) -> Result<InscribeResult, ApiError> {
    let signer_inputs = resolve_signer_inputs(options)?;
    let signer = build_signer(&signer_inputs)?;
    let from = signer.address();
    let to = resolve_to_address(options.to.as_deref(), from)?;

    let (prepared, estimate) = prepare_inscribe(
        client,
        &options.bytes,
        &PrepareInscribeOptions {
            source: options.source,
            optimize: options.optimize,
            gzip: options.gzip,
            esip6: options.esip6,
            skip_content_type: options.skip_content_type,
            data_uri_limit: 500,
        },
    )
    .await?;

    let exists = client.check_exists(&prepared.sha).await?;
    if exists.exists && !options.esip6 {
        return Err(ApiError::bad_request(format!(
            "Ethscription already exists for sha {}. Use esip6 to allow duplicates.",
            prepared.sha
        )));
    }

    if options.dry_run {
        return Ok(InscribeResult::DryRun(Box::new(DryRunInscribeResult {
            chain_id: normalized_chain_id(options.chain_id),
            content_type: prepared.content_type,
            data_uri: prepared.data_uri_truncated,
            exists: exists.exists,
            from: from.to_string(),
            gzipped_size: prepared.gzipped_size,
            mode: if options.blobs {
                "blobs".to_string()
            } else {
                "calldata".to_string()
            },
            sha: prepared.sha,
            size: prepared.size,
            to: to.to_string(),
            estimate: Some(estimate),
        })));
    }

    let provider = provider_with_signer(
        signer,
        signer_inputs
            .rpc_url
            .unwrap_or_else(|| "https://ethereum-rpc.publicnode.com".to_string()),
        normalized_chain_id(options.chain_id),
    )?;

    let request = build_transaction_request(
        from,
        to,
        normalized_chain_id(options.chain_id),
        prepared.data_uri.as_bytes(),
        options.blobs,
    )?;

    let pending = provider
        .send_transaction(request)
        .await
        .map_err(|e| ApiError::upstream_error(format!("Failed to submit transaction: {e}")))?;

    Ok(InscribeResult::Submitted(SubmittedInscribeResult {
        chain_id: normalized_chain_id(options.chain_id),
        content_type: prepared.content_type,
        from: from.to_string(),
        gzipped_size: prepared.gzipped_size,
        size: prepared.size,
        to: to.to_string(),
        transaction_hash: pending.tx_hash().to_string(),
    }))
}

fn normalized_chain_id(chain_id: u64) -> u64 {
    if chain_id == 0 { 1 } else { chain_id }
}

struct ResolvedSignerInputs {
    private_key: Option<String>,
    mnemonic: Option<String>,
    rpc_url: Option<String>,
}

fn resolve_signer_inputs(options: &InscribeOptions) -> Result<ResolvedSignerInputs, ApiError> {
    let env_file = if let Some(path) = &options.env_file {
        read_env_file(path)?
    } else {
        HashMap::new()
    };

    let private_key = options
        .private_key
        .clone()
        .or_else(|| env_file.get("ETHS_PRIVATE_KEY").cloned())
        .or_else(|| std::env::var("ETHS_PRIVATE_KEY").ok());
    let mnemonic = options
        .mnemonic
        .clone()
        .or_else(|| env_file.get("ETHS_MNEMONIC").cloned())
        .or_else(|| std::env::var("ETHS_MNEMONIC").ok());
    let rpc_url = options
        .rpc_url
        .clone()
        .or_else(|| env_file.get("ETHS_RPC_URL").cloned())
        .or_else(|| std::env::var("ETHS_RPC_URL").ok());

    if private_key.is_none() && mnemonic.is_none() {
        return Err(ApiError::bad_request(
            "Missing signer. Pass private_key or mnemonic, or set ETHS_PRIVATE_KEY / ETHS_MNEMONIC.",
        ));
    }

    Ok(ResolvedSignerInputs {
        private_key,
        mnemonic,
        rpc_url,
    })
}

fn build_signer(inputs: &ResolvedSignerInputs) -> Result<PrivateKeySigner, ApiError> {
    if let Some(private_key) = &inputs.private_key {
        let normalized = normalize_private_key(private_key);
        return normalized.parse().map_err(|e| {
            ApiError::bad_request(format!("Failed to parse private key signer: {e}"))
        });
    }

    if let Some(mnemonic) = &inputs.mnemonic {
        return MnemonicBuilder::<English>::default()
            .phrase(mnemonic)
            .build()
            .map_err(|e| ApiError::bad_request(format!("Failed to parse mnemonic signer: {e}")));
    }

    Err(ApiError::bad_request("No signer input provided"))
}

fn normalize_private_key(private_key: &str) -> String {
    if private_key.starts_with("0x") {
        private_key.to_string()
    } else {
        format!("0x{private_key}")
    }
}

fn resolve_to_address(value: Option<&str>, fallback: Address) -> Result<Address, ApiError> {
    match value {
        Some(value) => value
            .parse()
            .map_err(|e| ApiError::bad_request(format!("Invalid recipient address: {e}"))),
        None => Ok(fallback),
    }
}

fn provider_with_signer(
    signer: PrivateKeySigner,
    rpc_url: String,
    chain_id: u64,
) -> Result<impl Provider, ApiError> {
    let wallet = EthereumWallet::new(signer);
    let url = rpc_url
        .parse()
        .map_err(|e| ApiError::bad_request(format!("Invalid rpc_url: {e}")))?;

    Ok(ProviderBuilder::new()
        .with_chain_id(chain_id)
        .wallet(wallet)
        .connect_http(url))
}

fn build_transaction_request(
    from: Address,
    to: Address,
    chain_id: u64,
    data: &[u8],
    blobs: bool,
) -> Result<TransactionRequest, ApiError> {
    let mut request = TransactionRequest::default()
        .with_from(from)
        .with_to(to)
        .with_chain_id(chain_id);

    if blobs {
        let sidecar = build_blob_sidecar(data)?;
        request = request.with_blob_sidecar(sidecar);
    } else {
        request = request.with_input(data.to_vec());
    }

    Ok(request)
}

fn build_blob_sidecar(data: &[u8]) -> Result<BlobTransactionSidecar, ApiError> {
    SidecarBuilder::<SimpleCoder>::from_slice(data)
        .build_4844()
        .map_err(|e| ApiError::internal(format!("Failed to build blob sidecar: {e}")))
}

fn truncate_data_uri(data_uri: &str, limit: usize) -> String {
    let mut chars = data_uri.chars();
    let truncated: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn read_env_file(path: impl AsRef<Path>) -> Result<HashMap<String, String>, ApiError> {
    let content = fs::read_to_string(path.as_ref()).map_err(|e| {
        ApiError::bad_request(format!(
            "Failed to read env file '{}': {e}",
            path.as_ref().display()
        ))
    })?;
    let mut values = HashMap::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(
                key.trim().to_string(),
                strip_quotes(value.trim()).to_string(),
            );
        }
    }

    Ok(values)
}

fn strip_quotes(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}
