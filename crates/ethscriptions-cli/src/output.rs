// SPDX-License-Identifier: MPL-2.0

use ethscriptions_sdk::error::ApiError;
use ethscriptions_sdk::normalize::to_sorted_json_string;
use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::process;

#[derive(Debug)]
pub enum Output {
    Text(String),
    Bytes(Vec<u8>),
    File { path: String, bytes: Vec<u8> },
    Help(Box<clap::Command>),
    Failure(String),
}

pub fn json<T: Serialize>(value: &T, pretty: bool) -> String {
    format!("{}\n", to_sorted_json_string(value, pretty))
}

impl Output {
    pub fn json<T: Serialize>(value: &T, pretty: bool) -> Self {
        Self::Text(json(value, pretty))
    }

    pub fn render(self) -> Result<i32, Error> {
        match self {
            Self::Text(text) => print!("{text}"),
            Self::Bytes(bytes) => {
                let mut stdout = io::stdout().lock();
                stdout.write_all(&bytes).expect("Failed to write bytes");
                stdout.flush().expect("Failed to flush stdout");
            }
            Self::File { path, bytes } => {
                fs::write(&path, bytes).map_err(|e| format!("Failed to write to '{path}': {e}"))?
            }
            Self::Help(mut command) => {
                command.print_help()?;
                println!();
            }
            Self::Failure(text) => {
                print!("{text}");
                return Ok(1);
            }
        }
        Ok(0)
    }
}

#[derive(Debug)]
pub struct Error(pub String);

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Self(msg)
    }
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        Self(msg.to_string())
    }
}

impl From<ApiError> for Error {
    fn from(err: ApiError) -> Self {
        Self(err.to_string())
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self(err.to_string())
    }
}

/// Print an error message to stderr and exit with code 1.
pub fn die(msg: &str) -> ! {
    eprintln!("Error: {msg}");
    process::exit(1);
}
