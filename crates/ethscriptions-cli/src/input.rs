// SPDX-License-Identifier: MPL-2.0

use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

use ethscriptions_sdk::encode::InputSource;

pub struct Input {
    pub bytes: Vec<u8>,
    pub source: InputSource,
}

pub enum Stdin {
    Terminal,
    Piped(Box<dyn Read>),
}

impl Stdin {
    pub fn real() -> Self {
        let stdin = io::stdin();
        if stdin.is_terminal() {
            Self::Terminal
        } else {
            Self::Piped(Box::new(stdin))
        }
    }

    pub fn read_input(
        &mut self,
        file: Option<&PathBuf>,
        arg: Option<&str>,
    ) -> Result<Option<Input>, String> {
        if let Some(path) = file {
            let bytes = fs::read(path)
                .map_err(|e| format!("Failed to read file '{}': {e}", path.display()))?;
            return Ok(Some(Input {
                bytes,
                source: InputSource::File,
            }));
        }

        if let Self::Piped(reader) = self {
            let mut buf = Vec::new();
            reader
                .read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read stdin: {e}"))?;
            if !buf.is_empty() {
                return Ok(Some(Input {
                    bytes: buf,
                    source: InputSource::Stdin,
                }));
            }
        }

        if let Some(s) = arg {
            return Ok(Some(Input {
                bytes: s.as_bytes().to_vec(),
                source: InputSource::Literal,
            }));
        }

        Ok(None)
    }

    pub fn require_input(
        &mut self,
        file: Option<&PathBuf>,
        arg: Option<&str>,
        command_name: &str,
    ) -> Result<Input, String> {
        self.read_input(file, arg)?.ok_or_else(|| {
            format!("No input provided. Use --file <path>, pipe via stdin, or pass as argument.\nRun `eths {command_name} --help` for usage.")
        })
    }

    pub fn read_text_arg(&mut self, arg: Option<&str>) -> Result<Option<String>, String> {
        if let Some(s) = arg {
            return Ok(Some(s.to_string()));
        }

        if let Self::Piped(reader) = self {
            let mut buf = String::new();
            reader
                .read_to_string(&mut buf)
                .map_err(|e| format!("Failed to read stdin: {e}"))?;
            let trimmed = buf.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(Some(trimmed));
            }
        }

        Ok(None)
    }

    pub fn require_text_arg(
        &mut self,
        arg: Option<&str>,
        command_name: &str,
    ) -> Result<String, String> {
        self.read_text_arg(arg)?.ok_or_else(|| {
            format!(
                "No input provided. Pass as argument or pipe via stdin.\nRun `eths {command_name} --help` for usage."
            )
        })
    }
}

#[cfg(test)]
impl Stdin {
    pub fn piped(bytes: &'static [u8]) -> Self {
        Self::Piped(Box::new(bytes))
    }

    pub fn terminal() -> Self {
        Self::Terminal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
    }

    #[test]
    fn file_wins_over_stdin_and_arg() {
        let input = Stdin::piped(b"piped")
            .read_input(Some(&manifest()), Some("arg"))
            .unwrap()
            .unwrap();
        assert_eq!(input.source, InputSource::File);
        assert_eq!(input.bytes, fs::read(manifest()).unwrap());
    }

    #[test]
    fn piped_stdin_wins_over_arg() {
        let input = Stdin::piped(b"piped")
            .read_input(None, Some("arg"))
            .unwrap()
            .unwrap();
        assert_eq!(input.source, InputSource::Stdin);
        assert_eq!(input.bytes, b"piped");
    }

    #[test]
    fn empty_stdin_falls_back_to_arg() {
        let input = Stdin::piped(b"")
            .read_input(None, Some("arg"))
            .unwrap()
            .unwrap();
        assert_eq!(input.source, InputSource::Literal);
        assert_eq!(input.bytes, b"arg");
    }

    #[test]
    fn terminal_stdin_is_never_read() {
        let mut stdin = Stdin::terminal();
        assert!(stdin.read_input(None, None).unwrap().is_none());
        let err = stdin.require_input(None, None, "sha").err().unwrap();
        assert_eq!(
            err,
            "No input provided. Use --file <path>, pipe via stdin, or pass as argument.\nRun `eths sha --help` for usage."
        );
    }

    #[test]
    fn missing_file_names_the_path() {
        let err = Stdin::terminal()
            .read_input(Some(&PathBuf::from("nope.txt")), None)
            .err()
            .unwrap();
        assert!(err.starts_with("Failed to read file 'nope.txt': "), "{err}");
    }

    #[test]
    fn text_arg_wins_over_stdin() {
        let text = Stdin::piped(b"piped").read_text_arg(Some("arg")).unwrap();
        assert_eq!(text.as_deref(), Some("arg"));
    }

    #[test]
    fn text_from_stdin_is_trimmed() {
        let text = Stdin::piped(b"  0xabc \n").read_text_arg(None).unwrap();
        assert_eq!(text.as_deref(), Some("0xabc"));
    }

    #[test]
    fn blank_stdin_is_no_text() {
        let err = Stdin::piped(b" \n")
            .require_text_arg(None, "get")
            .err()
            .unwrap();
        assert_eq!(
            err,
            "No input provided. Pass as argument or pipe via stdin.\nRun `eths get --help` for usage."
        );
    }

    #[test]
    fn text_from_stdin_must_be_utf8() {
        let err = Stdin::piped(b"\xff").read_text_arg(None).err().unwrap();
        assert_eq!(
            err,
            "Failed to read stdin: stream did not contain valid UTF-8"
        );
    }
}
