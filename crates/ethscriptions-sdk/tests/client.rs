// SPDX-License-Identifier: MPL-2.0

//! `Client` endpoints against a local stub upstream.

#![allow(clippy::bool_assert_comparison)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use ethscriptions_sdk::cache::Cache;
use ethscriptions_sdk::client::Client;
use ethscriptions_sdk::fetcher::ContentData;

const HASH: &str = "0xabababababababababababababababababababababababababababababababab";
const SHA: &str = "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
const ADDR: &str = "0x1111111111111111111111111111111111111111";

type Routes = Arc<Mutex<HashMap<String, (u16, &'static str, Vec<u8>)>>>;

/// HTTP/1.1 server on a free local port. Answers each path with its set reply, else 404.
struct Stub {
    url: String,
    routes: Routes,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Stub {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let routes: Routes = Arc::default();
        let requests: Arc<Mutex<Vec<String>>> = Arc::default();
        let (thread_routes, thread_requests) = (routes.clone(), requests.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut head = Vec::new();
                let mut buf = [0u8; 1024];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => head.extend_from_slice(&buf[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&head);
                let target = head.split(' ').nth(1).unwrap_or("").to_string();
                thread_requests.lock().unwrap().push(target.clone());
                let path = target.split('?').next().unwrap_or("");
                let (status, content_type, body) = thread_routes
                    .lock()
                    .unwrap()
                    .get(path)
                    .cloned()
                    .unwrap_or((404, "text/plain", Vec::new()));
                let header = format!(
                    "HTTP/1.1 {status} Stub\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        Self {
            url,
            routes,
            requests,
        }
    }

    fn json(&self, path: &str, status: u16, body: &str) {
        self.raw(path, status, "application/json", body.as_bytes());
    }

    fn raw(&self, path: &str, status: u16, content_type: &'static str, body: &[u8]) {
        self.routes
            .lock()
            .unwrap()
            .insert(path.to_string(), (status, content_type, body.to_vec()));
    }

    fn take_requests(&self) -> Vec<String> {
        std::mem::take(&mut *self.requests.lock().unwrap())
    }
}

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "eths-sdk-client-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Client with every upstream URL on `stub` and the cache in a fresh temp dir.
fn client(stub: &Stub) -> Client {
    Client {
        api_url: stub.url.clone(),
        ens_url: format!("{}/ens", stub.url),
        gas_url: format!("{}/gas", stub.url),
        cache: Cache {
            root: Some(temp_dir()),
        },
        ..Client::new(None)
    }
}

/// URL of a local port that refuses connections.
fn dead_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    url
}

fn raw_ethscription(content_uri: &str) -> String {
    format!(
        r#"{{"transaction_hash":"{HASH}","block_number":19000000,"block_timestamp":1700000000,"transaction_index":7,"creator":"{ADDR}","initial_owner":"{ADDR}","current_owner":"{ADDR}","media_type":"text","mime_subtype":"plain","mimetype":"text/plain","content_sha":"0x{SHA}","content_uri":"{content_uri}","ethscription_number":"42"}}"#
    )
}

fn single() -> String {
    format!(r#"{{"result":{}}}"#, raw_ethscription("data:,hello"))
}

fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

const GAS: &str = r#"{"data":{"blockNr":"21000000","baseFee":10.5,"nextFee":11.25,"ethPrice":3000,"oracle":{"slow":{"gasFee":0.5,"priorityFee":0.1,"gwei":10},"normal":{"gasFee":0.75,"priorityFee":0.2,"gwei":12},"fast":{"gasFee":1.0,"priorityFee":0.5,"gwei":15}}}}"#;

#[tokio::test]
async fn fetch_ethscriptions_builds_query_and_maps_errors() {
    let stub = Stub::start();
    let client = client(&stub);
    let body = format!(
        r#"{{"result":[{}],"pagination":{{"has_more":true,"page_key":"k"}}}}"#,
        raw_ethscription("data:,hello")
    );
    stub.json("/ethscriptions", 200, &body);

    let list = client
        .fetch_ethscriptions(&params(&[("creator", ADDR), ("media_type", "text")]))
        .await
        .unwrap();
    assert_eq!(list.result.len(), 1);
    assert_eq!(list.result[0].transaction_hash, HASH);
    assert_eq!(list.pagination.unwrap().page_key.as_deref(), Some("k"));
    assert_eq!(
        stub.take_requests(),
        [format!("/ethscriptions?creator={ADDR}&media_type=text")]
    );

    stub.json("/ethscriptions", 500, "{}");
    let err = client.fetch_ethscriptions(&params(&[])).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "Upstream error: Upstream returned status 500 Internal Server Error"
    );

    stub.json("/ethscriptions", 200, "[]");
    let err = client.fetch_ethscriptions(&params(&[])).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "Internal error: Failed to parse upstream response: error decoding response body"
    );

    let dead = Client {
        api_url: dead_url(),
        ..client.clone()
    };
    let err = dead.fetch_ethscriptions(&params(&[])).await.unwrap_err();
    assert!(
        err.to_string()
            .starts_with("Upstream error: Fetch failed unexpectedly: error sending request"),
        "{err}"
    );
}

#[tokio::test]
async fn fetch_ethscriptions_resolves_names_through_ens() {
    let stub = Stub::start();
    let client = client(&stub);
    stub.json(
        "/ens/wgw.eth",
        200,
        &format!(r#"{{"address":"{}","ens":"wgw.eth"}}"#, ADDR.to_uppercase()),
    );
    stub.json("/ethscriptions", 200, r#"{"result":[]}"#);

    client
        .fetch_ethscriptions(&params(&[("creator", "wgw.eth"), ("resolve", "true")]))
        .await
        .unwrap();
    assert_eq!(
        stub.take_requests(),
        [
            "/ens/wgw.eth".to_string(),
            format!("/ethscriptions?creator={ADDR}")
        ]
    );
}

#[tokio::test]
async fn fetch_ethscription_uses_cache_and_owners_mode_skips_it() {
    let stub = Stub::start();
    let client = client(&stub);
    let path = format!("/ethscriptions/{HASH}");
    stub.json(&path, 200, &single());

    let (first, _) = client.fetch_ethscription(HASH).await.unwrap();
    stub.json(&path, 500, "{}");
    let (second, _) = client.fetch_ethscription(HASH).await.unwrap();
    assert_eq!(first.transaction_hash, second.transaction_hash);
    assert_eq!(stub.take_requests(), std::slice::from_ref(&path));

    let root = client.cache.root.clone().unwrap();
    assert_eq!(
        std::fs::read_dir(root.join("ethscription"))
            .unwrap()
            .count(),
        1
    );

    let err = client
        .fetch_ethscription_mode(HASH, Some("owners"))
        .await
        .err()
        .unwrap();
    assert_eq!(
        err.to_string(),
        "Not found: Transaction not found or it is not an Ethscription."
    );
    assert_eq!(stub.take_requests(), [path]);
}

#[tokio::test]
async fn cache_failure_aborts_the_fetch() {
    let stub = Stub::start();
    let blocker = temp_dir().join("blocker");
    std::fs::write(&blocker, "file").unwrap();
    let client = Client {
        cache: Cache {
            root: Some(blocker.clone()),
        },
        ..client(&stub)
    };
    stub.json(&format!("/ethscriptions/{HASH}"), 200, &single());

    let err = client.fetch_ethscription(HASH).await.unwrap_err();
    assert!(
        err.to_string().starts_with(&format!(
            "Internal error: Failed creating cache dir '{}'",
            blocker.join("ethscription").display()
        )),
        "{err}"
    );
}

#[tokio::test]
async fn check_exists_caches_only_hits() {
    let stub = Stub::start();
    let client = client(&stub);
    let path = format!("/ethscriptions/exists/0x{SHA}");

    let err = client.check_exists("nope").await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "Bad request: Invalid SHA-256 hash, must be 64 hex characters long, or 66 if 0x-prefixed"
    );

    stub.json(&path, 200, r#"{"result":{"exists":false}}"#);
    assert_eq!(client.check_exists(SHA).await.unwrap().exists, false);
    let hit = format!(
        r#"{{"result":{{"exists":true,"ethscription":{}}}}}"#,
        raw_ethscription("data:,hello")
    );
    stub.json(&path, 200, &hit);
    assert_eq!(client.check_exists(SHA).await.unwrap().exists, true);
    stub.json(&path, 500, "{}");
    let cached = client.check_exists(&format!("0x{SHA}")).await.unwrap();
    assert_eq!(cached.ethscription.unwrap().transaction_hash, HASH);
    assert_eq!(stub.take_requests(), [path.clone(), path]);
}

#[tokio::test]
async fn check_exists_maps_errors() {
    let stub = Stub::start();
    let client = client(&stub);
    stub.json(&format!("/ethscriptions/exists/0x{SHA}"), 503, "{}");
    let err = client.check_exists(SHA).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "Bad request: Failed checking if ethscription exists on the upstream API"
    );

    let dead = Client {
        api_url: dead_url(),
        ..client
    };
    let err = dead.check_exists(SHA).await.unwrap_err();
    assert!(
        err.to_string().starts_with(
            "Internal error: Failed to fetch from upstream API: error sending request"
        ),
        "{err}"
    );
}

#[tokio::test]
async fn content_and_attachment() {
    let stub = Stub::start();
    let client = client(&stub);
    stub.raw("/blob", 200, "image/png", b"png");
    let (data, content_type) = client
        .fetch_ethscription_content(&format!("{}/blob", stub.url), Some(true), None)
        .await
        .unwrap();
    assert_eq!(data.as_base64(), "cG5n");
    assert_eq!(content_type, "image/png");

    stub.raw("/blob", 404, "text/plain", b"");
    let err = client
        .fetch_ethscription_content(&format!("{}/blob", stub.url), None, None)
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Upstream error: Failed to fetch content: upstream returned status 404 Not Found"
    );

    let attachment = format!("/ethscriptions/{HASH}/attachment");
    stub.raw(&attachment, 200, "application/octet-stream", b"blob");
    let data = client
        .fetch_ethscription_attachment(HASH, None, None)
        .await
        .unwrap();
    assert!(matches!(data, ContentData::Bytes(ref b) if b == b"blob"));

    stub.raw(&attachment, 500, "text/plain", b"");
    let err = client
        .fetch_ethscription_attachment(HASH, None, None)
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Not found: Attachment not found for this ethscription."
    );
}

#[tokio::test]
async fn fetch_recent_pulse_reads_transfers() {
    let stub = Stub::start();
    let client = client(&stub);
    let body = format!(
        r#"{{"result":[{{"block_blockhash":"0xb1","block_number":5,"block_timestamp":"1700000000","ethscription_transaction_hash":"{HASH}","from_address":"{ADDR}","to_address":"{ADDR}","transaction_hash":"0xfe","transaction_index":1,"transfer_index":"0"}}]}}"#
    );
    stub.json("/ethscription_transfers", 200, &body);
    let pulse = client
        .fetch_recent_pulse(&params(&[("page_size", "1")]))
        .await
        .unwrap();
    assert_eq!(pulse.result[0].event_type, "transfer");
    assert_eq!(
        stub.take_requests(),
        ["/ethscription_transfers?max_results=1"]
    );
}

#[tokio::test]
async fn gas_prices_and_estimate_use_gas_url() {
    let stub = Stub::start();
    let client = client(&stub);
    stub.json("/gas", 200, GAS);

    let prices = client.fetch_gas_prices("fast").await.unwrap();
    assert_eq!(prices.block_number, "21000000");
    assert_eq!(prices.base_fee, "10.5");
    assert_eq!(prices.gas_price, "15");
    let normal = client.fetch_gas_prices("normal").await.unwrap();
    assert_eq!(normal.gas_price, "12");

    let estimate = client
        .estimate_data_cost(b"data:,hi", "fast", 0)
        .await
        .unwrap();
    assert_eq!(estimate.prices.gas_price, "15");
    assert_eq!(estimate.meta.input_length, "8");

    stub.json("/gas", 503, "{}");
    let err = client.fetch_gas_prices("fast").await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "Upstream error: Failed to fetch gas prices: 503 Service Unavailable"
    );

    let dead = Client {
        gas_url: dead_url(),
        ..client
    };
    let err = dead.fetch_gas_prices("fast").await.unwrap_err();
    assert!(
        err.to_string()
            .starts_with("Internal error: Failed to fetch gas prices: error sending request"),
        "{err}"
    );
}

#[tokio::test]
async fn resolve_user_tries_ens_then_ethscription_names() {
    let stub = Stub::start();
    let client = client(&stub);
    stub.json(
        "/ens/wgw.eth",
        200,
        &format!(r#"{{"address":"{ADDR}","ens":"WGW.eth"}}"#),
    );
    let resolved = client.resolve_user("wgw.eth", false).await.unwrap();
    assert_eq!(resolved.address, ADDR);
    assert_eq!(resolved.name, "wgw.eth");

    let name_sha = ethscriptions_sdk::digest::compute_name_digest("Tester");
    let hit = format!(
        r#"{{"result":{{"exists":true,"ethscription":{}}}}}"#,
        raw_ethscription("data:,Tester")
    );
    stub.json(&format!("/ethscriptions/exists/0x{name_sha}"), 200, &hit);
    stub.take_requests();
    let resolved = client.resolve_user("Tester", false).await.unwrap();
    assert_eq!(resolved.address, ADDR);
    assert_eq!(
        stub.take_requests(),
        [
            "/ens/tester".to_string(),
            format!("/ethscriptions/exists/0x{name_sha}")
        ]
    );

    let err = client.resolve_user("nobody", false).await.unwrap_err();
    assert_eq!(err.to_string(), "Not found: Cannot resolve nobody");
}
