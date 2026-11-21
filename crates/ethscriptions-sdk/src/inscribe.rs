// SPDX-License-Identifier: MPL-2.0

use alloy::eips::eip4844::BlobTransactionSidecar;
use alloy::eips::eip4844::builder::{SidecarBuilder, SimpleCoder};
use alloy::network::{EthereumWallet, TransactionBuilder, TransactionBuilder4844};
use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy_signer_local::{MnemonicBuilder, PrivateKeySigner, coins_bip39::English};
use serde::Serialize;

use crate::client::Client;
use crate::encode::prepare_payload;
pub use crate::encode::{PrepareInscribeOptions, PreparedInscribe};
use crate::error::ApiError;
use crate::types::EstimateCostResult;

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
    pub prepare: PrepareInscribeOptions,
    pub blobs: bool,
    pub dry_run: bool,
    pub to: Option<String>,
    pub chain_id: u64,
}

pub async fn prepare_inscribe(
    client: &Client,
    bytes: &[u8],
    options: &PrepareInscribeOptions,
) -> Result<(PreparedInscribe, EstimateCostResult), ApiError> {
    let prepared = prepare_payload(bytes, options)?;
    let estimate = client
        .estimate_data_cost(prepared.data_uri.as_bytes(), "fast", 0)
        .await?;
    Ok((prepared, estimate))
}

pub async fn inscribe(
    client: &Client,
    signer: PrivateKeySigner,
    rpc_url: &str,
    options: &InscribeOptions,
) -> Result<InscribeResult, ApiError> {
    let from = signer.address();
    let to = resolve_to_address(options.to.as_deref(), from)?;

    let (prepared, estimate) = prepare_inscribe(client, &options.bytes, &options.prepare).await?;

    let exists = client.check_exists(&prepared.sha).await?;
    if exists.exists && !options.prepare.esip6 {
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

    let provider = provider_with_signer(signer, rpc_url, normalized_chain_id(options.chain_id))?;

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

pub fn private_key_signer(private_key: &str) -> Result<PrivateKeySigner, ApiError> {
    normalize_private_key(private_key)
        .parse()
        .map_err(|e| ApiError::bad_request(format!("Failed to parse private key signer: {e}")))
}

pub fn mnemonic_signer(mnemonic: &str) -> Result<PrivateKeySigner, ApiError> {
    MnemonicBuilder::<English>::default()
        .phrase(mnemonic)
        .build()
        .map_err(|e| ApiError::bad_request(format!("Failed to parse mnemonic signer: {e}")))
}

fn normalized_chain_id(chain_id: u64) -> u64 {
    if chain_id == 0 { 1 } else { chain_id }
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
    rpc_url: &str,
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
