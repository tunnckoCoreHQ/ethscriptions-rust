// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;
use std::process::{Command, Stdio};

const KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
const KEY_ADDRESS: &str = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8";
const MNEMONIC: &str = "test test test test test test test test test test test junk";
const MNEMONIC_ADDRESS: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

const BAD_KEY: &str = "zz";
const BAD_MNEMONIC: &str = "foo bar";

const CLOSED_PORT_FLAG_URL: &str = "http://127.0.0.1:1/flag";
const CLOSED_PORT_ENV_URL: &str = "http://127.0.0.1:1/env";
const CLOSED_PORT_FILE_URL: &str = "http://127.0.0.1:1/file";

const BAD_KEY_ERROR: &str =
    "Error: Bad request: Failed to parse private key signer: invalid string length\n";
const BAD_MNEMONIC_ERROR: &str =
    "Error: Bad request: Failed to parse mnemonic signer: the word `foo` is invalid\n";
const BAD_TO_ERROR: &str = "Error: Bad request: Invalid recipient address: odd number of digits\n";
const MISSING_SIGNER_ERROR: &str = "Error: Bad request: Missing signer. Pass private_key or mnemonic, or set ETHS_PRIVATE_KEY / ETHS_MNEMONIC.\n";

struct Output {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn eths(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_eths"));
    command
        .arg("inscribe")
        .args(args)
        .env_remove("ETHS_PRIVATE_KEY")
        .env_remove("ETHS_MNEMONIC")
        .env_remove("ETHS_RPC_URL")
        .env_remove("ETHS_BASE_URL")
        .envs(envs.iter().copied())
        .stdin(Stdio::null());
    let output = command.output().expect("run eths");
    Output {
        code: output.status.code(),
        stdout: String::from_utf8(output.stdout).expect("utf-8 stdout"),
        stderr: String::from_utf8(output.stderr).expect("utf-8 stderr"),
    }
}

fn env_file(name: &str, content: &str) -> String {
    let path: PathBuf =
        std::env::temp_dir().join(format!("eths-inscribe-{}-{name}.env", std::process::id()));
    std::fs::write(&path, content).expect("write env file");
    path.to_string_lossy().into_owned()
}

fn assert_error(output: &Output, stderr: &str) {
    assert_eq!(output.stderr, stderr);
    assert_eq!(output.stdout, "");
    assert_eq!(output.code, Some(1));
}

#[test]
fn missing_input_fails_before_the_signer() {
    let output = eths(&["--env-file", "/nonexistent/eths.env"], &[]);
    assert_error(
        &output,
        "Error: No input provided. Use --file <path>, pipe via stdin, or pass as argument.\nRun `eths inscribe --help` for usage.\n",
    );
}

#[test]
fn missing_signer() {
    assert_error(&eths(&["hi"], &[]), MISSING_SIGNER_ERROR);
}

#[test]
fn rpc_url_alone_is_not_a_signer() {
    let file = env_file(
        "rpc-only",
        &format!("ETHS_RPC_URL={CLOSED_PORT_FILE_URL}\n"),
    );
    let output = eths(
        &["hi", "--env-file", &file, "--rpc-url", CLOSED_PORT_FLAG_URL],
        &[("ETHS_RPC_URL", CLOSED_PORT_ENV_URL)],
    );
    assert_error(&output, MISSING_SIGNER_ERROR);
}

#[test]
fn unreadable_env_file_fails_even_with_a_key_flag() {
    let output = eths(
        &[
            "hi",
            "--env-file",
            "/nonexistent/eths.env",
            "--private-key",
            KEY,
        ],
        &[],
    );
    assert_error(
        &output,
        "Error: Bad request: Failed to read env file '/nonexistent/eths.env': No such file or directory (os error 2)\n",
    );
}

#[test]
fn private_key_flag_beats_env_file_and_env_var() {
    let file = env_file("pk-flag-bad", &format!("ETHS_PRIVATE_KEY={KEY}\n"));
    let output = eths(
        &["hi", "--env-file", &file, "--private-key", BAD_KEY],
        &[("ETHS_PRIVATE_KEY", KEY)],
    );
    assert_error(&output, BAD_KEY_ERROR);

    let file = env_file("pk-flag-good", &format!("ETHS_PRIVATE_KEY={BAD_KEY}\n"));
    let output = eths(
        &["hi", "--env-file", &file, "--private-key", KEY, "--to", "x"],
        &[("ETHS_PRIVATE_KEY", BAD_KEY)],
    );
    assert_error(&output, BAD_TO_ERROR);
}

#[test]
fn secret_key_alias_sets_the_private_key() {
    assert_error(&eths(&["hi", "--secret-key", BAD_KEY], &[]), BAD_KEY_ERROR);
}

#[test]
fn private_key_env_var_beats_env_file() {
    let file = env_file("pk-env-bad", &format!("ETHS_PRIVATE_KEY={KEY}\n"));
    let output = eths(
        &["hi", "--env-file", &file],
        &[("ETHS_PRIVATE_KEY", BAD_KEY)],
    );
    assert_error(&output, BAD_KEY_ERROR);

    let file = env_file("pk-env-good", &format!("ETHS_PRIVATE_KEY={BAD_KEY}\n"));
    let output = eths(
        &["hi", "--env-file", &file, "--to", "x"],
        &[("ETHS_PRIVATE_KEY", KEY)],
    );
    assert_error(&output, BAD_TO_ERROR);
}

#[test]
fn private_key_env_var_alone() {
    assert_error(
        &eths(&["hi"], &[("ETHS_PRIVATE_KEY", BAD_KEY)]),
        BAD_KEY_ERROR,
    );
    assert_error(
        &eths(&["hi", "--to", "x"], &[("ETHS_PRIVATE_KEY", KEY)]),
        BAD_TO_ERROR,
    );
}

#[test]
fn private_key_without_0x_prefix_is_accepted() {
    let output = eths(&["hi", "--private-key", &KEY[2..], "--to", "x"], &[]);
    assert_error(&output, BAD_TO_ERROR);
}

#[test]
fn empty_private_key_env_var_counts_as_a_key() {
    let output = eths(
        &["hi", "--mnemonic", MNEMONIC, "--to", "x"],
        &[("ETHS_PRIVATE_KEY", "")],
    );
    assert_error(
        &output,
        "Error: Bad request: Failed to parse private key signer: invalid string length\n",
    );
}

#[test]
fn mnemonic_flag_beats_env_file_and_env_var() {
    let file = env_file("mn-flag-bad", &format!("ETHS_MNEMONIC={MNEMONIC}\n"));
    let output = eths(
        &["hi", "--env-file", &file, "--mnemonic", BAD_MNEMONIC],
        &[("ETHS_MNEMONIC", MNEMONIC)],
    );
    assert_error(&output, BAD_MNEMONIC_ERROR);

    let file = env_file("mn-flag-good", &format!("ETHS_MNEMONIC={BAD_MNEMONIC}\n"));
    let output = eths(
        &[
            "hi",
            "--env-file",
            &file,
            "--mnemonic",
            MNEMONIC,
            "--to",
            "x",
        ],
        &[("ETHS_MNEMONIC", BAD_MNEMONIC)],
    );
    assert_error(&output, BAD_TO_ERROR);
}

#[test]
fn mnemonic_env_var_beats_env_file() {
    let file = env_file("mn-env-bad", &format!("ETHS_MNEMONIC={MNEMONIC}\n"));
    let output = eths(
        &["hi", "--env-file", &file],
        &[("ETHS_MNEMONIC", BAD_MNEMONIC)],
    );
    assert_error(&output, BAD_MNEMONIC_ERROR);

    let file = env_file("mn-env-good", &format!("ETHS_MNEMONIC={BAD_MNEMONIC}\n"));
    let output = eths(
        &["hi", "--env-file", &file, "--to", "x"],
        &[("ETHS_MNEMONIC", MNEMONIC)],
    );
    assert_error(&output, BAD_TO_ERROR);
}

#[test]
fn mnemonic_env_var_alone() {
    assert_error(
        &eths(&["hi"], &[("ETHS_MNEMONIC", BAD_MNEMONIC)]),
        BAD_MNEMONIC_ERROR,
    );
    assert_error(
        &eths(&["hi", "--to", "x"], &[("ETHS_MNEMONIC", MNEMONIC)]),
        BAD_TO_ERROR,
    );
}

#[test]
fn private_key_from_any_source_beats_mnemonic_from_any_source() {
    let output = eths(
        &["hi", "--mnemonic", MNEMONIC],
        &[("ETHS_PRIVATE_KEY", BAD_KEY)],
    );
    assert_error(&output, BAD_KEY_ERROR);

    let file = env_file("pk-over-mn", &format!("ETHS_PRIVATE_KEY={BAD_KEY}\n"));
    let output = eths(&["hi", "--env-file", &file, "--mnemonic", MNEMONIC], &[]);
    assert_error(&output, BAD_KEY_ERROR);

    let output = eths(
        &[
            "hi",
            "--private-key",
            KEY,
            "--mnemonic",
            BAD_MNEMONIC,
            "--to",
            "x",
        ],
        &[],
    );
    assert_error(&output, BAD_TO_ERROR);
}

#[test]
fn env_file_parsing() {
    let content = format!(
        "# comment\n\n   \nETHS_PRIVATE_KEY={BAD_KEY}\nnot a pair\n  ETHS_PRIVATE_KEY = \"{KEY}\"  \n"
    );
    let file = env_file("parse-dq", &content);
    assert_error(
        &eths(&["hi", "--env-file", &file, "--to", "x"], &[]),
        BAD_TO_ERROR,
    );

    let file = env_file("parse-sq", &format!("ETHS_PRIVATE_KEY='{KEY}'\n"));
    assert_error(
        &eths(&["hi", "--env-file", &file, "--to", "x"], &[]),
        BAD_TO_ERROR,
    );

    let file = env_file("parse-mixed", &format!("ETHS_PRIVATE_KEY=\"{KEY}'\n"));
    assert_error(
        &eths(&["hi", "--env-file", &file, "--to", "x"], &[]),
        "Error: Bad request: Failed to parse private key signer: invalid string length\n",
    );

    let file = env_file("parse-export", &format!("export ETHS_PRIVATE_KEY={KEY}\n"));
    assert_error(
        &eths(&["hi", "--env-file", &file], &[]),
        MISSING_SIGNER_ERROR,
    );

    let file = env_file("parse-commented", &format!("#ETHS_PRIVATE_KEY={KEY}\n"));
    assert_error(
        &eths(&["hi", "--env-file", &file], &[]),
        MISSING_SIGNER_ERROR,
    );
}

fn submit_error(url: &str) -> String {
    format!(
        "Error: Upstream error: Failed to submit transaction: error sending request for url ({url})\n"
    )
}

const SUBMIT: [&str; 4] = ["hi", "--esip6", "--private-key", KEY];

#[test]
fn dry_run_ignores_the_rpc_url() {
    let output = eths(
        &[
            "hi",
            "--esip6",
            "--mnemonic",
            MNEMONIC,
            "--dry-run",
            "--rpc-url",
            "not a url",
            "--chain-id",
            "0",
        ],
        &[],
    );
    assert_eq!(output.stderr, "");
    assert_eq!(output.code, Some(0));
    let json: serde_json::Value = serde_json::from_str(&output.stdout).expect("json");
    assert_eq!(json["from"], MNEMONIC_ADDRESS);
    assert_eq!(json["to"], MNEMONIC_ADDRESS);
    assert_eq!(json["chain_id"], 1);
    assert_eq!(json["mode"], "calldata");
    assert_eq!(json["data_uri"], "data:text/plain;rule=esip6,hi");
}

#[test]
fn dry_run_reports_the_key_address() {
    let mut args = SUBMIT.to_vec();
    args.extend(["--dry-run", "--blobs"]);
    let output = eths(&args, &[]);
    assert_eq!(output.code, Some(0));
    let json: serde_json::Value = serde_json::from_str(&output.stdout).expect("json");
    assert_eq!(json["from"], KEY_ADDRESS);
    assert_eq!(json["mode"], "blobs");
}

#[test]
fn rpc_url_flag_beats_env_var_and_env_file() {
    let file = env_file(
        "rpc-flag",
        &format!("ETHS_RPC_URL={CLOSED_PORT_FILE_URL}\n"),
    );
    let mut args = SUBMIT.to_vec();
    args.extend(["--env-file", &file, "--rpc-url", CLOSED_PORT_FLAG_URL]);
    let output = eths(&args, &[("ETHS_RPC_URL", CLOSED_PORT_ENV_URL)]);
    assert_error(&output, &submit_error(CLOSED_PORT_FLAG_URL));
}

#[test]
fn rpc_url_env_var_beats_env_file() {
    let file = env_file("rpc-env", &format!("ETHS_RPC_URL={CLOSED_PORT_FILE_URL}\n"));
    let mut args = SUBMIT.to_vec();
    args.extend(["--env-file", &file]);
    let output = eths(&args, &[("ETHS_RPC_URL", CLOSED_PORT_ENV_URL)]);
    assert_error(&output, &submit_error(CLOSED_PORT_ENV_URL));
}

#[test]
fn rpc_url_env_file_is_used_alone() {
    let file = env_file(
        "rpc-file",
        &format!("ETHS_RPC_URL={CLOSED_PORT_FILE_URL}\n"),
    );
    let mut args = SUBMIT.to_vec();
    args.extend(["--env-file", &file]);
    let output = eths(&args, &[]);
    assert_error(&output, &submit_error(CLOSED_PORT_FILE_URL));
}

#[test]
fn rpc_url_empty_env_var_beats_env_file() {
    let file = env_file(
        "rpc-empty",
        &format!("ETHS_RPC_URL={CLOSED_PORT_FILE_URL}\n"),
    );
    let mut args = SUBMIT.to_vec();
    args.extend(["--env-file", &file]);
    let output = eths(&args, &[("ETHS_RPC_URL", "")]);
    assert_error(
        &output,
        "Error: Bad request: Invalid rpc_url: relative URL without a base\n",
    );
}

#[test]
fn rpc_url_empty_env_var_alone_is_invalid() {
    let output = eths(&SUBMIT, &[("ETHS_RPC_URL", "")]);
    assert_error(
        &output,
        "Error: Bad request: Invalid rpc_url: relative URL without a base\n",
    );
}
