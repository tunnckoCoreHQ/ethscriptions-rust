// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::{Args, Subcommand};
use ethscriptions_sdk::validate;
use serde::Serialize;

use crate::input::Stdin;
use crate::output::{Error, Output, json};

#[derive(Args)]
#[command(next_display_order = None)]
pub struct ValidateArgs {
    #[command(subcommand)]
    pub kind: ValidateKind,

    /// Output JSON instead of plain text true/false
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum ValidateKind {
    /// Validate an Ethereum address (0x + 40 hex chars)
    Address {
        /// The value to validate
        value: Option<String>,
    },
    /// Validate a transaction hash (0x + 64 hex chars)
    Hash {
        /// The value to validate
        value: Option<String>,
    },
    /// Validate a hex string (with or without 0x prefix)
    Hex {
        /// The value to validate
        value: Option<String>,
        /// Require 0x prefix
        #[arg(long)]
        prefixed: bool,
    },
    /// Validate a SHA-256 hash (64 hex chars, no prefix)
    Sha256 {
        /// The value to validate
        value: Option<String>,
        /// Read input from a file (computes SHA-256, then validates the hash)
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Validate a data URI
    DataUri {
        /// The value to validate
        value: Option<String>,
        /// Read input from a file (validates as data URI content)
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Validate an ethscription ID (hash or numeric)
    Id {
        /// The value to validate
        value: Option<String>,
    },
}

#[derive(Serialize)]
struct ValidateOutput {
    valid: bool,
    value: String,
    kind: String,
}

pub fn run(args: ValidateArgs, stdin: &mut Stdin, pretty: bool) -> Result<Output, Error> {
    let (valid, value, kind) = match &args.kind {
        ValidateKind::Address { value } => {
            let val = stdin
                .read_text_arg(value.as_deref())?
                .ok_or("No value provided. Pass as argument or pipe via stdin.")?;
            (validate::is_valid_address(&val), val, "address")
        }
        ValidateKind::Hash { value } => {
            let val = stdin
                .read_text_arg(value.as_deref())?
                .ok_or("No value provided. Pass as argument or pipe via stdin.")?;
            (validate::is_valid_hash(&val), val, "hash")
        }
        ValidateKind::Hex { value, prefixed } => {
            let val = stdin
                .read_text_arg(value.as_deref())?
                .ok_or("No value provided. Pass as argument or pipe via stdin.")?;
            (validate::is_valid_hex(&val, *prefixed), val, "hex")
        }
        ValidateKind::Sha256 { value, file } => {
            let val = if file.is_some() {
                let bytes = stdin
                    .require_input(file.as_ref(), None, "validate sha256")?
                    .bytes;
                String::from_utf8(bytes)
                    .map_err(|_| "File content is not valid UTF-8 text".to_string())?
                    .trim()
                    .to_string()
            } else {
                stdin
                    .read_text_arg(value.as_deref())?
                    .ok_or("No value provided. Pass as argument, --file, or pipe via stdin.")?
            };
            (validate::is_valid_sha256(&val), val, "sha256")
        }
        ValidateKind::DataUri { value, file } => {
            let val = if file.is_some() {
                let bytes = stdin
                    .require_input(file.as_ref(), None, "validate data-uri")?
                    .bytes;
                String::from_utf8(bytes)
                    .map_err(|_| "File content is not valid UTF-8 text".to_string())?
                    .trim()
                    .to_string()
            } else {
                stdin
                    .read_text_arg(value.as_deref())?
                    .ok_or("No value provided. Pass as argument, --file, or pipe via stdin.")?
            };
            (validate::is_valid_data_uri(&val), val, "data-uri")
        }
        ValidateKind::Id { value } => {
            let val = stdin
                .read_text_arg(value.as_deref())?
                .ok_or("No value provided. Pass as argument or pipe via stdin.")?;
            (validate::parse_ethscription_id(&val).is_some(), val, "id")
        }
    };

    let text = if args.json {
        let output = ValidateOutput {
            valid,
            value,
            kind: kind.to_string(),
        };
        json(&output, pretty)
    } else {
        format!("{valid}\n")
    };

    Ok(if valid {
        Output::Text(text)
    } else {
        Output::Failure(text)
    })
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::{Cli, Commands};

    fn validate(argv: &[&str], mut stdin: Stdin) -> Output {
        let Commands::Validate(args) = Cli::parse_from(argv).command else {
            panic!("not a validate command");
        };
        run(args, &mut stdin, false).unwrap()
    }

    #[test]
    fn valid_value_prints_true() {
        let address = "0x2052051a0474fb0b98283b3f38c13b0b0b6a3677";
        let output = validate(&["eths", "validate", "address", address], Stdin::terminal());
        assert!(matches!(output, Output::Text(text) if text == "true\n"));
    }

    #[test]
    fn invalid_value_prints_false_and_fails() {
        let output = validate(&["eths", "validate", "hash", "0x12"], Stdin::terminal());
        assert!(matches!(output, Output::Failure(text) if text == "false\n"));
    }

    #[test]
    fn piped_value_is_trimmed_in_json() {
        let output = validate(
            &["eths", "validate", "--json", "id"],
            Stdin::piped(b" nope \n"),
        );
        assert!(matches!(
            output,
            Output::Failure(text) if text == "{\"kind\":\"id\",\"valid\":false,\"value\":\"nope\"}\n"
        ));
    }
}
