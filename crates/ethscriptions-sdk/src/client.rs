// SPDX-License-Identifier: MPL-2.0

use bytes::Bytes;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;

use crate::cache::Cache;
use crate::constants::{BASE_API_URL, ENS_API_URL, GAS_API_URL, RPC_URL};

/// Client for the upstream APIs: the HTTP client, the base URLs, and the cache.
#[derive(Debug, Clone)]
pub struct Client {
    pub http: reqwest::Client,
    /// Ethscriptions API base URL.
    pub api_url: String,
    /// ENS data API base URL.
    pub ens_url: String,
    /// Gas prices API URL.
    pub gas_url: String,
    /// Default Ethereum JSON-RPC URL, used when no RPC URL is configured.
    pub rpc_url: String,
    pub cache: Cache,
}

pub(crate) enum Failure {
    Send(reqwest::Error),
    Status(StatusCode),
    Body(reqwest::Error),
}

impl Client {
    /// Client for the public upstreams. `api_url` overrides the Ethscriptions API base URL.
    pub fn new(api_url: Option<&str>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_url: api_url.unwrap_or(BASE_API_URL).to_string(),
            ens_url: ENS_API_URL.to_string(),
            gas_url: GAS_API_URL.to_string(),
            rpc_url: RPC_URL.to_string(),
            cache: Cache::default(),
        }
    }

    async fn get(&self, url: &str) -> Result<reqwest::Response, Failure> {
        let resp = self.http.get(url).send().await.map_err(Failure::Send)?;
        if !resp.status().is_success() {
            return Err(Failure::Status(resp.status()));
        }
        Ok(resp)
    }

    pub(crate) async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, Failure> {
        self.get(url).await?.json().await.map_err(Failure::Body)
    }

    pub(crate) async fn get_bytes(&self, url: &str) -> Result<(String, Bytes), Failure> {
        let resp = self.get(url).await?;
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = resp.bytes().await.map_err(Failure::Body)?;
        Ok((content_type, bytes))
    }
}
