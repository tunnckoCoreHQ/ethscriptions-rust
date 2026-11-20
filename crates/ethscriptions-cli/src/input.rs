// SPDX-License-Identifier: MPL-2.0

use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

/// Read input bytes from a file, stdin, or a positional argument string.
///
/// Priority:
/// 1. `--file <path>` -> read file bytes
/// 2. Piped stdin (non-terminal) -> read stdin bytes
/// 3. Positional arg -> treat as literal string bytes
///
/// Returns `None` if no input source is available.
pub fn read_input(file: Option<&PathBuf>, arg: Option<&str>) -> Result<Option<Vec<u8>>, String> {
    // 1. File takes priority
    if let Some(path) = file {
        let bytes =
            fs::read(path).map_err(|e| format!("Failed to read file '{}': {e}", path.display()))?;
        return Ok(Some(bytes));
    }

    // 2. Piped stdin (non-interactive)
    if !io::stdin().is_terminal() {
        let mut buf = Vec::new();
        io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| format!("Failed to read stdin: {e}"))?;
        if !buf.is_empty() {
            return Ok(Some(buf));
        }
    }

    // 3. Positional argument as literal string
    if let Some(s) = arg {
        return Ok(Some(s.as_bytes().to_vec()));
    }

    Ok(None)
}

/// Same as `read_input` but returns an error if no input is available.
pub fn require_input(
    file: Option<&PathBuf>,
    arg: Option<&str>,
    command_name: &str,
) -> Result<Vec<u8>, String> {
    read_input(file, arg)?.ok_or_else(|| {
        format!("No input provided. Use --file <path>, pipe via stdin, or pass as argument.\nRun `eths {command_name} --help` for usage.")
    })
}

/// Read a single text value from a positional arg or piped stdin (trimmed).
///
/// Use this for commands that accept an ID, name, hash, etc. as a positional
/// argument but should also accept piped input (e.g. from another command).
///
/// Priority:
/// 1. Positional arg (if provided)
/// 2. Piped stdin (read as text, trimmed)
///
/// Returns `None` if no input source is available.
pub fn read_text_arg(arg: Option<&str>) -> Result<Option<String>, String> {
    // 1. Positional arg takes priority
    if let Some(s) = arg {
        return Ok(Some(s.to_string()));
    }

    // 2. Piped stdin
    if !io::stdin().is_terminal() {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("Failed to read stdin: {e}"))?;
        let trimmed = buf.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed));
        }
    }

    Ok(None)
}

/// Same as `read_text_arg` but returns an error if no input is available.
pub fn require_text_arg(arg: Option<&str>, command_name: &str) -> Result<String, String> {
    read_text_arg(arg)?.ok_or_else(|| {
        format!(
            "No input provided. Pass as argument or pipe via stdin.\nRun `eths {command_name} --help` for usage."
        )
    })
}
