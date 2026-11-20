// SPDX-License-Identifier: MPL-2.0

use bytes::Bytes;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;

use crate::constants::{BASE_API_URL, ENS_API_URL};

/// Client for the upstream APIs: the HTTP client and the base URLs.
///
/// Endpoints are methods on it, in `fetcher` and `resolve`.
#[derive(Debug, Clone)]
pub struct Client {
    pub http: reqwest::Client,
    /// Ethscriptions API base URL.
    pub api_url: String,
    /// ENS data API base URL.
    pub ens_url: String,
}

/// Why a GET to an upstream failed. Each endpoint maps it to its own error.
pub(crate) enum Failure {
    /// The request could not be sent.
    Send(reqwest::Error),
    /// The upstream answered with a non-success status.
    Status(StatusCode),
    /// The response body could not be read or parsed.
    Body(reqwest::Error),
}

impl Client {
    /// Client for the public upstreams. `api_url` overrides the Ethscriptions API base URL.
    pub fn new(api_url: Option<&str>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_url: api_url.unwrap_or(BASE_API_URL).to_string(),
            ens_url: ENS_API_URL.to_string(),
        }
    }

    async fn get(&self, url: &str) -> Result<reqwest::Response, Failure> {
        let resp = self.http.get(url).send().await.map_err(Failure::Send)?;
        if !resp.status().is_success() {
            return Err(Failure::Status(resp.status()));
        }
        Ok(resp)
    }

    /// GET `url` and parse the JSON body.
    pub(crate) async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, Failure> {
        self.get(url).await?.json().await.map_err(Failure::Body)
    }

    /// GET `url` and return its content type (`application/octet-stream` when missing) and body.
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
