// SPDX-License-Identifier: MPL-2.0

/// Check if a string is a valid Ethereum address (0x-prefixed, 42 chars).
pub fn is_valid_address(val: &str) -> bool {
    val.starts_with("0x") && is_valid_hex(val, true) && val.len() == 42
}

/// Check if a string is valid hex (even length, all hex digits).
/// If `prefixed` is true, the string must start with `0x`.
pub fn is_valid_hex(val: &str, prefixed: bool) -> bool {
    if val.is_empty() {
        return false;
    }

    let hex_part = if prefixed {
        if !val.starts_with("0x") {
            return false;
        }
        &val[2..]
    } else {
        val
    };

    hex_part.len() % 2 == 0 && hex_part.chars().all(|c| c.is_ascii_hexdigit())
}

/// Check if a string is a valid transaction hash (64 hex chars, or 66 if 0x-prefixed).
/// Automatically detects whether the `0x` prefix is present.
pub fn is_valid_hash(val: &str) -> bool {
    let has_prefix = val.starts_with("0x");
    let expected_len = if has_prefix { 66 } else { 64 };

    val.len() == expected_len && is_valid_hex(val, has_prefix)
}

/// Check if a string is a valid data URI.
///
/// Validates the structure: `data:[<mediatype>][;base64],<data>`
/// - Must start with `data:`
/// - Must contain a comma (the comma separates metadata from data)
/// - If a mediatype is present, it must contain a `/` (e.g. `text/plain`)
/// - If `;base64` is present, it must appear right before the comma
pub fn is_valid_data_uri(val: &str) -> bool {
    let Some(rest) = val.strip_prefix("data:") else {
        return false;
    };

    // Must contain a comma separating metadata from data
    let Some(comma_pos) = rest.find(',') else {
        return false;
    };

    let metadata = &rest[..comma_pos];

    // Empty metadata is valid (e.g. `data:,hello` = text/plain;charset=US-ASCII)
    if metadata.is_empty() {
        return true;
    }

    // Split metadata into parts by semicolons
    let parts: Vec<&str> = metadata.split(';').collect();
    let mediatype = parts[0];

    // If mediatype is non-empty, it must contain a slash (e.g. text/plain)
    if !mediatype.is_empty() && !mediatype.contains('/') {
        return false;
    }

    // If there are parameters, the last one can be "base64"
    // Other parameters should be key=value pairs
    for part in &parts[1..] {
        let trimmed = part.trim();
        if trimmed == "base64" {
            continue;
        }
        // Parameters should be key=value (e.g. charset=utf-8)
        if !trimmed.contains('=') {
            return false;
        }
    }

    true
}

/// Parse an ethscription ID which can be:
/// - A transaction hash (0x + 64 hex chars)
/// - A numeric string (ethscription number)
/// - A BigInt representation (number) of the transaction hash
///
/// Returns the ID as-is if valid, or `None` if invalid.
pub fn parse_ethscription_id(val: &str) -> Option<String> {
    if val.is_empty() {
        return None;
    }

    // Transaction hash (must be 0x-prefixed)
    if val.starts_with("0x") && is_valid_hash(val) {
        return Some(val.to_string());
    }

    // Pure numeric
    if val.chars().all(|c| c.is_ascii_digit()) {
        return Some(val.to_string());
    }

    // // Hex without 0x prefix (could be a BigInt hex representation)
    // if val.chars().all(|c| c.is_ascii_hexdigit()) && val.len() % 2 == 0 {
    //     return Some(val.to_string());
    // }

    None
}

/// Check if a string is a valid SHA-256 hash (64 hex chars, optionally 0x-prefixed).
pub fn is_valid_sha256(val: &str) -> bool {
    let has_prefix = val.starts_with("0x");
    let expected_len = if has_prefix { 66 } else { 64 };

    val.len() == expected_len && is_valid_hex(val, has_prefix)
}

/// Convert hex string to bytes.
pub fn hex_to_bytes(hex: &str) -> Vec<u8> {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0))
        .collect()
}

/// Convert bytes to hex string (no 0x prefix).
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_address() {
        assert_eq!(
            is_valid_address("0x1234567890abcdef1234567890abcdef12345678"),
            true
        );
        assert_eq!(
            is_valid_address("1234567890abcdef1234567890abcdef12345678"),
            false
        ); // no 0x
        assert_eq!(
            is_valid_address("0x1234567890aBARef1234567890abcdef12345678"),
            false
        ); // invalid hex
        assert_eq!(is_valid_address("0x1234"), false); // too short
        assert_eq!(is_valid_address(""), false); // empty
    }

    #[test]
    fn test_is_valid_hex() {
        assert_eq!(is_valid_hex("abcdef", false), true);
        assert_eq!(is_valid_hex("0xabcdef", true), true);
        assert_eq!(is_valid_hex("abcdef", true), false); // missing 0x prefix
        assert_eq!(is_valid_hex("0xabcdef", false), false); // has 0x but prefixed=false (odd hex part)
        assert_eq!(is_valid_hex("abcde", false), false); // odd length
        assert_eq!(is_valid_hex("foobarqx", false), false); // invalid hex
        assert_eq!(is_valid_hex("", false), false); // empty
        assert_eq!(is_valid_hex("xyz", false), false); // non-hex
    }

    #[test]
    fn test_is_valid_hash() {
        let valid_prefixed = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let valid_not_prefixed = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        assert_eq!(is_valid_hash(valid_prefixed), true); // 0x-prefixed, 66 chars
        assert_eq!(is_valid_hash(valid_not_prefixed), true); // no prefix, 64 chars
        assert_eq!(is_valid_hash("0x1234"), false); // too short
        assert_eq!(is_valid_hash("1234"), false); // too short
        assert_eq!(
            is_valid_hash("0x1234567890aFBARf1234567890abcdef12345678"),
            false
        ); // invalid hex
        assert_eq!(
            is_valid_hash("1234567890aFBARf1234567890abcdef12345678aabbccdd1234567890abcdef"),
            false
        ); // invalid hex, no prefix
    }

    #[test]
    fn test_is_valid_data_uri() {
        // Valid URIs
        assert_eq!(is_valid_data_uri("data:,hello"), true);
        assert_eq!(is_valid_data_uri("data:,"), true); // empty data is valid
        assert_eq!(is_valid_data_uri("data:text/plain,hello"), true);
        assert_eq!(is_valid_data_uri("data:text/plain;base64,SGVsbG8="), true);
        assert_eq!(
            is_valid_data_uri("data:text/plain;charset=utf-8,hello"),
            true
        );
        assert_eq!(
            is_valid_data_uri("data:text/plain;charset=utf-8;base64,SGVsbG8="),
            true
        );
        assert_eq!(is_valid_data_uri("data:image/png;base64,iVBOR"), true);
        assert_eq!(
            is_valid_data_uri("data:application/json,{\"key\":\"value\"}"),
            true
        );

        // Invalid URIs
        assert_eq!(is_valid_data_uri("http://example.com"), false);
        assert_eq!(is_valid_data_uri(""), false);
        assert_eq!(is_valid_data_uri("data:"), false); // no comma
        assert_eq!(is_valid_data_uri("data:noslash,hello"), false); // mediatype without /
        assert_eq!(is_valid_data_uri("data:text/plain;badparam,hello"), false); // param without = and not base64
    }

    #[test]
    fn test_parse_ethscription_id() {
        // Valid hash
        let hash = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        assert_eq!(parse_ethscription_id(hash), Some(hash.to_string()));

        // Numeric (ethscription number)
        assert_eq!(parse_ethscription_id("12345"), Some("12345".to_string()));

        // Numeric (transaction hash / ethscription id, as big int number)
        assert_eq!(
            parse_ethscription_id(
                "55250016227810986664382831318994040593618016366818392656099097336638803798865"
            ),
            Some(
                "55250016227810986664382831318994040593618016366818392656099097336638803798865"
                    .to_string()
            )
        );

        // Invalid
        assert_eq!(parse_ethscription_id(""), None);
        assert_eq!(parse_ethscription_id("not-valid!"), None);
    }

    #[test]
    fn test_is_valid_sha256() {
        let valid = "a".repeat(64);
        assert_eq!(is_valid_sha256(&valid), true);
        assert_eq!(is_valid_sha256(&format!("0x{valid}")), true);
        assert_eq!(is_valid_sha256("tooshort"), false);
        assert_eq!(is_valid_sha256(&"g".repeat(64)), false); // non-hex
    }

    #[test]
    fn test_hex_to_bytes() {
        assert_eq!(hex_to_bytes("48656c6c6f"), b"Hello".to_vec());
        assert_eq!(hex_to_bytes("0x48656c6c6f"), b"Hello".to_vec());
    }

    #[test]
    fn test_bytes_to_hex() {
        assert_eq!(bytes_to_hex(b"Hello"), "48656c6c6f");
    }
}
