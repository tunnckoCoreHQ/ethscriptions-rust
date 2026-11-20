// SPDX-License-Identifier: MPL-2.0

//! Runs the `eths` binary against a local stub upstream and compares exit code,
//! stdout, stderr, the requests sent, and the cache files with snapshots.
//! Set `UPDATE_SNAPSHOTS=1` to rewrite the snapshots.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const HASH: &str = "0xabababababababababababababababababababababababababababababababab";
const SHA: &str = "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
const ADDR: &str = "0x1111111111111111111111111111111111111111";

#[derive(Clone)]
struct Reply {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

fn json(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "application/json",
        body: body.as_bytes().to_vec(),
    }
}

fn raw(status: u16, content_type: &'static str, body: &[u8]) -> Reply {
    Reply {
        status,
        content_type,
        body: body.to_vec(),
    }
}

struct Stub {
    url: String,
    routes: Arc<Mutex<HashMap<String, Reply>>>,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Stub {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let routes: Arc<Mutex<HashMap<String, Reply>>> = Arc::default();
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
                let mut line = head.lines().next().unwrap_or("").split(' ');
                let (method, target) = (line.next().unwrap_or(""), line.next().unwrap_or(""));
                thread_requests
                    .lock()
                    .unwrap()
                    .push(format!("{method} {target}"));
                let path = target.split('?').next().unwrap_or("");
                let reply = thread_routes
                    .lock()
                    .unwrap()
                    .get(path)
                    .cloned()
                    .unwrap_or_else(|| raw(404, "text/plain", b""));
                let header = format!(
                    "HTTP/1.1 {} Stub\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    reply.status,
                    reply.content_type,
                    reply.body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&reply.body);
            }
        });
        Self {
            url,
            routes,
            requests,
        }
    }

    fn set(&self, path: &str, reply: Reply) {
        self.routes.lock().unwrap().insert(path.to_string(), reply);
    }

    fn take_requests(&self) -> Vec<String> {
        std::mem::take(&mut *self.requests.lock().unwrap())
    }
}

fn dead_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    url
}

fn temp_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "eths-upstream-{name}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Environment for one child run. `None` removes the variable.
type Env<'a> = &'a [(&'a str, Option<&'a str>)];

struct Case {
    name: &'static str,
    tmp: PathBuf,
    log: String,
}

impl Case {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            tmp: temp_dir(name),
            log: String::new(),
        }
    }

    fn cache_dir(&self) -> PathBuf {
        self.tmp.join("cache")
    }

    /// Run `eths` with `ETHS_CACHE_DIR` set to this case's cache dir.
    fn run(&mut self, stub: Option<&Stub>, args: &[&str]) {
        let cache = self.cache_dir();
        let cache = cache.to_str().unwrap().to_string();
        self.run_env(stub, args, &[("ETHS_CACHE_DIR", Some(&cache))]);
    }

    fn run_env(&mut self, stub: Option<&Stub>, args: &[&str], env: Env) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_eths"));
        cmd.args(args)
            .stdin(Stdio::null())
            .env("HOME", self.tmp.join("home"))
            .env("NO_PROXY", "*");
        for key in [
            "ETHS_BASE_URL",
            "ETHS_CACHE_DIR",
            "XDG_CACHE_HOME",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ] {
            cmd.env_remove(key);
        }
        for (key, value) in env {
            match value {
                Some(value) => cmd.env(key, value),
                None => cmd.env_remove(key),
            };
        }
        let out = cmd.output().unwrap();
        let requests = stub.map(Stub::take_requests).unwrap_or_default();
        self.log.push_str(&format!(
            "$ eths {}\nexit: {}\n--- stdout\n{}\n--- stderr\n{}\n--- requests\n{}\n--- files\n{}\n\n",
            args.join(" "),
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
            requests.join("\n"),
            list_files(&self.tmp).join("\n"),
        ));
    }

    /// Compare the log with the snapshot. Each of `urls` is a base URL used in
    /// the case: its cache keys become `sha256(<key>)` and its port `PORT`.
    fn check(self, urls: &[&str]) {
        let mut text = self.log.replace(self.tmp.to_str().unwrap(), "$TMP");
        for url in urls {
            for key in [
                format!("{url}|ethscription|{HASH}"),
                format!("{url}|exists|{SHA}"),
            ] {
                text = text.replace(&sha256(&key), &format!("sha256({key})"));
            }
            text = text.replace(url, "http://127.0.0.1:PORT");
        }
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/snapshots")
            .join(format!("{}.snap", self.name));
        if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, &text).unwrap();
        } else {
            let expected = fs::read_to_string(&path).unwrap_or_default();
            assert_eq!(text, expected, "snapshot {} differs", self.name);
        }
        let _ = fs::remove_dir_all(&self.tmp);
    }
}

fn list_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
    }
    out.sort();
    out
}

fn find_cache_file(root: &Path) -> PathBuf {
    let files = list_files(root);
    let file = files.iter().find(|f| f.ends_with(".json")).unwrap();
    root.join(file)
}

fn raw_ethscription(content_uri: &str) -> String {
    format!(
        r#"{{
  "transaction_hash": "{HASH}",
  "block_number": 19000000,
  "block_blockhash": "0xb10c",
  "block_timestamp": 1700000000,
  "transaction_index": 7,
  "event_log_index": null,
  "value": "0",
  "transaction_fee": "1000",
  "gas_price": "10",
  "gas_used": "100",
  "creator": "{ADDR}",
  "initial_owner": "0x2222222222222222222222222222222222222222",
  "current_owner": "0x3333333333333333333333333333333333333333",
  "previous_owner": "0x2222222222222222222222222222222222222222",
  "media_type": "text",
  "mime_subtype": "plain",
  "mimetype": "text/plain",
  "content_sha": "0x{SHA}",
  "content_uri": "{content_uri}",
  "esip6": false,
  "ethscription_number": "42",
  "ethscription_transfers": [
    {{
      "ethscription_transaction_hash": "{HASH}",
      "transaction_hash": "{HASH}",
      "from_address": "{ADDR}",
      "to_address": "0x2222222222222222222222222222222222222222",
      "block_number": 19000000,
      "block_timestamp": 1700000000,
      "block_blockhash": "0xb10c",
      "event_log_index": null,
      "transaction_index": 7
    }},
    {{
      "ethscription_transaction_hash": "{HASH}",
      "transaction_hash": "0xfefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefe",
      "from_address": "0x2222222222222222222222222222222222222222",
      "to_address": "0x3333333333333333333333333333333333333333",
      "block_number": 19000100,
      "block_timestamp": 1700001200,
      "block_blockhash": "0xb10d",
      "event_log_index": 3,
      "transaction_index": 2,
      "enforced_previous_owner": "0x2222222222222222222222222222222222222222"
    }}
  ]
}}"#
    )
}

fn single(content_uri: &str) -> String {
    format!(r#"{{"result": {}}}"#, raw_ethscription(content_uri))
}

fn list_body(content_uri: &str) -> String {
    format!(
        r#"{{"result": [{}], "pagination": {{"has_more": true, "page_key": "next-key"}}}}"#,
        raw_ethscription(content_uri)
    )
}

fn single_path() -> String {
    format!("/ethscriptions/{HASH}")
}

#[test]
fn get_caches_by_base_url_and_id() {
    let stub = Stub::start();
    let mut case = Case::new("get_cache");
    stub.set(&single_path(), json(200, &single("data:,hello")));
    case.run(Some(&stub), &["--base-url", &stub.url, "get", HASH]);
    stub.set(&single_path(), json(500, "{}"));
    case.run(Some(&stub), &["--base-url", &stub.url, "get", HASH]);
    case.run(
        Some(&stub),
        &["--base-url", &stub.url, "get", HASH, "--only", "creator"],
    );
    case.run(
        Some(&stub),
        &["--base-url", &stub.url, "get", "--numbers", HASH],
    );
    case.run(Some(&stub), &["--base-url", &stub.url, "transfers", HASH]);
    case.check(&[&stub.url]);
}

#[test]
fn get_expired_cache_refetches() {
    let stub = Stub::start();
    let mut case = Case::new("get_expired_cache");
    stub.set(&single_path(), json(200, &single("data:,hello")));
    case.run(Some(&stub), &["--base-url", &stub.url, "get", HASH]);
    let file = find_cache_file(&case.cache_dir());
    let text = fs::read_to_string(&file).unwrap();
    let rest = text.split_once(',').unwrap().1;
    fs::write(&file, format!(r#"{{"expires_at":1,{rest}"#)).unwrap();
    case.run(Some(&stub), &["--base-url", &stub.url, "get", HASH]);
    case.check(&[&stub.url]);
}

#[test]
fn get_corrupt_cache_fails() {
    let stub = Stub::start();
    let mut case = Case::new("get_corrupt_cache");
    stub.set(&single_path(), json(200, &single("data:,hello")));
    case.run(Some(&stub), &["--base-url", &stub.url, "get", HASH]);
    let file = find_cache_file(&case.cache_dir());
    fs::write(&file, "not json").unwrap();
    case.run(Some(&stub), &["--base-url", &stub.url, "get", HASH]);
    case.check(&[&stub.url]);
}

#[test]
fn get_errors() {
    let stub = Stub::start();
    let dead = dead_url();
    let mut case = Case::new("get_errors");
    stub.set(&single_path(), json(404, r#"{"error":"nope"}"#));
    case.run(Some(&stub), &["--base-url", &stub.url, "get", HASH]);
    stub.set(&single_path(), json(200, "not json"));
    case.run(Some(&stub), &["--base-url", &stub.url, "get", HASH]);
    case.run(None, &["--base-url", &dead, "get", HASH]);
    case.run(None, &["--base-url", &dead, "get", "--owners", HASH]);
    case.run(None, &["--base-url", &dead, "get", "nope"]);
    case.check(&[&stub.url, &dead]);
}

#[test]
fn get_owners_skips_cache() {
    let stub = Stub::start();
    let mut case = Case::new("get_owners");
    stub.set(&single_path(), json(200, &single("data:,hello")));
    case.run(
        Some(&stub),
        &["--base-url", &stub.url, "get", "--owners", HASH],
    );
    case.run(
        Some(&stub),
        &["--base-url", &stub.url, "get", "--owners", HASH],
    );
    stub.set(&single_path(), json(503, "{}"));
    case.run(
        Some(&stub),
        &["--base-url", &stub.url, "get", "--owners", HASH],
    );
    case.check(&[&stub.url]);
}

#[test]
fn base_url_from_env() {
    let stub = Stub::start();
    let mut case = Case::new("base_url_env");
    stub.set(&single_path(), json(200, &single("data:,hello")));
    let cache = case.cache_dir().to_str().unwrap().to_string();
    case.run_env(
        Some(&stub),
        &["get", "--only", "transaction_hash", HASH],
        &[
            ("ETHS_BASE_URL", Some(&stub.url)),
            ("ETHS_CACHE_DIR", Some(&cache)),
        ],
    );
    case.check(&[&stub.url]);
}

#[test]
fn cache_dir_fallbacks() {
    let stub = Stub::start();
    let mut case = Case::new("cache_dirs");
    stub.set(&single_path(), json(200, &single("data:,hello")));
    let xdg = case.tmp.join("xdg").to_str().unwrap().to_string();
    let args = ["--base-url", &stub.url, "get", "--only", "creator", HASH];
    case.run_env(Some(&stub), &args, &[("XDG_CACHE_HOME", Some(&xdg))]);
    case.run_env(Some(&stub), &args, &[]);
    case.run_env(Some(&stub), &args, &[("HOME", None)]);
    let blocker = case.tmp.join("blocker");
    fs::write(&blocker, "file").unwrap();
    let blocker = blocker.to_str().unwrap().to_string();
    case.run_env(Some(&stub), &args, &[("ETHS_CACHE_DIR", Some(&blocker))]);
    case.check(&[&stub.url]);
}

#[test]
fn list_requests_and_errors() {
    let stub = Stub::start();
    let dead = dead_url();
    let mut case = Case::new("list");
    stub.set("/ethscriptions", json(200, &list_body("data:,hello")));
    case.run(Some(&stub), &["--base-url", &stub.url, "list"]);
    case.run(
        Some(&stub),
        &[
            "--base-url",
            &stub.url,
            "list",
            "--creator",
            ADDR,
            "--media-type",
            "text",
            "--page-size",
            "2",
            "--reverse",
            "--only",
            "transaction_hash",
        ],
    );
    stub.set("/ethscriptions", json(500, "{}"));
    case.run(Some(&stub), &["--base-url", &stub.url, "list"]);
    stub.set("/ethscriptions", json(200, "[]"));
    case.run(Some(&stub), &["--base-url", &stub.url, "list"]);
    case.run(None, &["--base-url", &dead, "list"]);
    case.check(&[&stub.url, &dead]);
}

#[test]
fn exists_caches_only_hits() {
    let stub = Stub::start();
    let dead = dead_url();
    let mut case = Case::new("exists");
    let path = format!("/ethscriptions/exists/0x{SHA}");
    stub.set(&path, json(200, r#"{"result":{"exists":false}}"#));
    case.run(Some(&stub), &["--base-url", &stub.url, "exists", SHA]);
    case.run(Some(&stub), &["--base-url", &stub.url, "exists", SHA]);
    let hit = format!(
        r#"{{"result":{{"exists":true,"ethscription":{}}}}}"#,
        raw_ethscription("data:,hello")
    );
    stub.set(&path, json(200, &hit));
    case.run(
        Some(&stub),
        &["--base-url", &stub.url, "exists", &format!("0x{SHA}")],
    );
    stub.set(&path, json(500, "{}"));
    case.run(Some(&stub), &["--base-url", &stub.url, "exists", SHA]);
    case.check(&[&stub.url, &dead]);
}

#[test]
fn exists_errors() {
    let stub = Stub::start();
    let dead = dead_url();
    let mut case = Case::new("exists_errors");
    let path = format!("/ethscriptions/exists/0x{SHA}");
    stub.set(&path, json(500, "{}"));
    case.run(Some(&stub), &["--base-url", &stub.url, "exists", SHA]);
    stub.set(&path, json(200, "nope"));
    case.run(Some(&stub), &["--base-url", &stub.url, "exists", SHA]);
    case.run(None, &["--base-url", &dead, "exists", SHA]);
    case.run(Some(&stub), &["--base-url", &stub.url, "exists", "hello"]);
    case.check(&[&stub.url, &dead]);
}

#[test]
fn content_and_attachment() {
    let stub = Stub::start();
    let dead = dead_url();
    let mut case = Case::new("content");
    let content_url = format!("{}/blob/one", stub.url);
    stub.set(&single_path(), json(200, &single(&content_url)));
    stub.set("/blob/one", raw(200, "image/png", b"\x89PNG-bytes"));
    case.run(Some(&stub), &["--base-url", &stub.url, "content", HASH]);
    case.run(
        Some(&stub),
        &["--base-url", &stub.url, "content", "--base64", HASH],
    );
    stub.set("/blob/one", raw(404, "text/plain", b"gone"));
    case.run(Some(&stub), &["--base-url", &stub.url, "content", HASH]);

    let attachment = format!("/ethscriptions/{HASH}/attachment");
    stub.set(
        &attachment,
        raw(200, "application/octet-stream", b"attached"),
    );
    case.run(Some(&stub), &["--base-url", &stub.url, "attachment", HASH]);
    case.run(
        Some(&stub),
        &["--base-url", &stub.url, "attachment", "--base64", HASH],
    );
    stub.set(&attachment, raw(500, "text/plain", b"x"));
    case.run(Some(&stub), &["--base-url", &stub.url, "attachment", HASH]);
    case.run(None, &["--base-url", &dead, "attachment", HASH]);
    case.check(&[&stub.url, &dead]);
}

#[test]
fn content_dead_and_data_uri() {
    let stub = Stub::start();
    let dead = dead_url();
    let mut case = Case::new("content_dead");
    let other = Stub::start();
    other.set(
        &single_path(),
        json(200, &single(&format!("{dead}/blob/two"))),
    );
    case.run(Some(&other), &["--base-url", &other.url, "content", HASH]);
    stub.set(
        &single_path(),
        json(200, &single("data:text/plain,hi%20there")),
    );
    case.run(Some(&stub), &["--base-url", &stub.url, "content", HASH]);
    case.check(&[&stub.url, &other.url, &dead]);
}

#[test]
fn recent_pulse() {
    let stub = Stub::start();
    let dead = dead_url();
    let mut case = Case::new("recent");
    let body = format!(
        r#"{{"result":[{{"block_blockhash":"0xb1","block_number":5,"block_timestamp":"1700000000","ethscription_transaction_hash":"{HASH}","event_log_index":null,"from_address":"{ADDR}","to_address":"{ADDR}","transaction_hash":"{HASH}","transaction_index":1,"transfer_index":"0"}}],"pagination":{{"has_more":false,"page_key":null}}}}"#
    );
    stub.set("/ethscription_transfers", json(200, &body));
    case.run(Some(&stub), &["--base-url", &stub.url, "recent"]);
    case.run(
        Some(&stub),
        &[
            "--base-url",
            &stub.url,
            "recent",
            "--from",
            ADDR,
            "--page-size",
            "1",
        ],
    );
    stub.set("/ethscription_transfers", json(502, "{}"));
    case.run(Some(&stub), &["--base-url", &stub.url, "recent"]);
    stub.set("/ethscription_transfers", json(200, "{}"));
    case.run(Some(&stub), &["--base-url", &stub.url, "recent"]);
    case.run(None, &["--base-url", &dead, "recent"]);
    case.check(&[&stub.url, &dead]);
}

#[test]
fn profile() {
    let stub = Stub::start();
    let mut case = Case::new("profile");
    let content_url = format!("{}/profile.json", stub.url);
    stub.set("/ethscriptions", json(200, &list_body(&content_url)));
    stub.set(
        "/profile.json",
        raw(200, "application/json", br#"{"name":"tester","bio":"hi"}"#),
    );
    case.run(Some(&stub), &["--base-url", &stub.url, "profile", ADDR]);
    case.run(
        Some(&stub),
        &["--base-url", &stub.url, "profile", "--info", ADDR],
    );
    case.run(
        Some(&stub),
        &[
            "--base-url",
            &stub.url,
            "profile",
            "--created",
            ADDR,
            "--page-size",
            "3",
        ],
    );
    case.run(
        Some(&stub),
        &["--base-url", &stub.url, "profile", "--owned", ADDR],
    );
    stub.set("/ethscriptions", json(200, r#"{"result":[]}"#));
    case.run(Some(&stub), &["--base-url", &stub.url, "profile", ADDR]);
    case.check(&[&stub.url]);
}

fn sha256(text: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(text.as_bytes()))
}
