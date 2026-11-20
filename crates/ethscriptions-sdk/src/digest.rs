// SPDX-License-Identifier: MPL-2.0

use sha2::{Digest, Sha256};

use crate::validate::{bytes_to_hex, hex_to_bytes, is_valid_hex};

/// Compute SHA-256 digest of a message, returned as a lowercase hex string (no 0x prefix).
pub fn compute_digest(msg: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(msg);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Compute SHA-256 digest for a `data:,{name}` URI used in Ethscription Name resolution.
pub fn compute_name_digest(name: &str) -> String {
    let uri = format!("data:,{name}");
    compute_digest(uri.as_bytes())
}

/// Compute digest for various input types (data URI string, hex-encoded data URI, raw bytes).
///
/// Returns `(sha256_hex, data_hex, decoded_input_string)` or an error message.
pub fn compute_data_digest(input: &str) -> Result<(String, String, String), String> {
    let is_raw_data = input.starts_with("data:");
    let stripped = input.strip_prefix("0x").unwrap_or(input);
    let is_hex_data = is_valid_hex(stripped, false) && stripped.starts_with("646174613a");

    if !is_raw_data && !is_hex_data {
        return Err(
            "Invalid data, must be valid DataURI or hex-encoded DataURI string".to_string(),
        );
    }

    let data: Vec<u8> = if is_raw_data {
        input.as_bytes().to_vec()
    } else {
        hex_to_bytes(stripped)
    };

    let sha = compute_digest(&data);
    let hex_str = bytes_to_hex(&data);
    let decoded = String::from_utf8_lossy(&data).to_string();

    Ok((sha, hex_str, decoded))
}

/// Compute digest from raw bytes (e.g. a `Uint8Array` equivalent).
pub fn compute_bytes_digest(data: &[u8]) -> (String, String, String) {
    let sha = compute_digest(data);
    let hex_str = bytes_to_hex(data);
    let decoded = String::from_utf8_lossy(data).to_string();
    (sha, hex_str, decoded)
}

/// Estimate the gas cost for inscribing data.
///
/// Each zero byte costs 4 gas, each non-zero byte costs 16 gas, plus 21000 for the transfer.
pub fn estimate_gas(data: &[u8], buffer_fee: u64) -> u64 {
    let data_gas: u64 = data
        .iter()
        .map(|&byte| if byte == 0 { 4u64 } else { 16u64 })
        .sum();
    data_gas + 21_000 + buffer_fee
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_digest_known_value() {
        // SHA-256 of "hello" = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        let result = compute_digest(b"hello");
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_compute_digest_empty() {
        // SHA-256 of "" = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let result = compute_digest(b"");
        assert_eq!(
            result,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_compute_name_digest() {
        // SHA-256 of "data:,hello"
        let result = compute_name_digest("hello");
        let expected = compute_digest(b"data:,hello");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_compute_data_digest_raw_data_uri() {
        let (sha, hex, decoded) = compute_data_digest("data:,hello").unwrap();
        assert_eq!(sha, compute_digest(b"data:,hello"));
        assert_eq!(hex.is_empty(), false);
        assert_eq!(decoded, "data:,hello");
    }

    #[test]
    fn test_compute_data_digest_hex_encoded() {
        // "data:,hello" in hex = 646174613a2c68656c6c6f
        let hex_input = "646174613a2c68656c6c6f";
        let (sha, _hex, decoded) = compute_data_digest(hex_input).unwrap();
        assert_eq!(sha, compute_digest(b"data:,hello"));
        assert_eq!(decoded, "data:,hello");

        let hex_input_prefixed = "0x646174613a2c68656c6c6f";
        let (sha2, _hex2, decoded2) = compute_data_digest(hex_input_prefixed).unwrap();
        assert_eq!(sha2, compute_digest(b"data:,hello"));
        assert_eq!(decoded2, "data:,hello");
    }

    #[test]
    fn test_compute_data_digest_invalid_input() {
        let result = compute_data_digest("not a data uri");
        assert_eq!(result.is_err(), true);
    }

    #[test]
    fn test_estimate_gas_basic() {
        // 5 non-zero bytes = 5 * 16 = 80, + 21000 = 21080
        let gas = estimate_gas(b"hello", 0);
        assert_eq!(gas, 21_080);
    }

    #[test]
    fn test_estimate_gas_with_zeros() {
        // [0, 1, 0, 2] = 4 + 16 + 4 + 16 = 40, + 21000 = 21040
        let gas = estimate_gas(&[0, 1, 0, 2], 0);
        assert_eq!(gas, 21_040);
    }

    #[test]
    fn test_estimate_gas_with_buffer() {
        let gas = estimate_gas(b"hello", 100);
        assert_eq!(gas, 21_180); // 80 + 21000 + 100
    }
}
