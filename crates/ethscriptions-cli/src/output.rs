// SPDX-License-Identifier: MPL-2.0

use ethscriptions_sdk::normalize::to_sorted_json_string;
use serde::Serialize;
use std::io::{self, Write};
use std::process;

/// Print a serializable value as JSON to stdout with sorted keys for determinism.
pub fn print_json<T: Serialize>(value: &T, pretty: bool) {
    let json = to_sorted_json_string(value, pretty);
    println!("{json}");
}

/// Print raw bytes to stdout (for binary content).
pub fn print_bytes(bytes: &[u8]) {
    let mut stdout = io::stdout().lock();
    stdout.write_all(bytes).expect("Failed to write bytes");
    stdout.flush().expect("Failed to flush stdout");
}

/// Write raw bytes to a file.
pub fn write_to_file(bytes: &[u8], path: &str) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("Failed to write to '{path}': {e}"))
}

/// Print an error message to stderr and exit with code 1.
pub fn die(msg: &str) -> ! {
    eprintln!("Error: {msg}");
    process::exit(1);
}
