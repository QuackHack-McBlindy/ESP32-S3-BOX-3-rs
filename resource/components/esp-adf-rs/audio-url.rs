#![no_std]
extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::result::Result;

/// URL encoding/decoding errors.
#[derive(Debug, PartialEq)]
pub enum UrlError {
    /// Memory allocation failed.
    AllocError,
    /// Invalid percent-encoded sequence (e.g., missing or non-hex digits).
    InvalidEncoding,
}

/// Converts a 4‑bit nibble to its hexadecimal ASCII character.
const fn char_to_hex(x: u8) -> u8 {
    if x > 9 {
        x + 0x37  // 'A' - 10
    } else {
        x + 0x30  // '0'
    }
}

/// Percent‑encodes a string according to standard URL encoding rules.
///
/// Only alphanumeric characters and the following safe characters are left
/// unchanged: `- _ . ! @ # $ & * ( ) = : / , ; ? + ' ~`
/// All other bytes are replaced by `%XX` where `XX` is the hexadecimal value.
///
/// Returns `Err(UrlError::AllocError)` if the output string cannot be allocated.
pub fn url_encode(input: &str) -> Result<String, UrlError> {
    let bytes = input.as_bytes();
    // Upper bound: each byte expands to at most 3 bytes ('%' + 2 hex digits)
    let capacity = bytes.len() * 3;
    let mut output = Vec::with_capacity(capacity);
    if output.capacity() < capacity {
        return Err(UrlError::AllocError);
    }

    for &b in bytes {
        if b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'_' | b'.' | b'!' | b'@' | b'#' | b'$' | b'&' | b'*'
                            | b'(' | b')' | b'=' | b':' | b'/' | b',' | b';' | b'?'
                            | b'+' | b'\'' | b'~')
        {
            output.push(b);
        } else {
            output.push(b'%');
            output.push(char_to_hex(b >> 4));
            output.push(char_to_hex(b & 0x0F));
        }
    }

    // Safety: the Vec contains valid UTF-8 because we only appended
    // ASCII bytes (alnum, safe chars, or '%' and hex digits).
    String::from_utf8(output).map_err(|_| UrlError::InvalidEncoding)
}

/// Decodes a percent‑encoded string.
///
/// Every `%XX` sequence is replaced by the byte with hexadecimal value `XX`.
/// Non‑`%` characters are copied verbatim.
///
/// Returns `Err(UrlError::InvalidEncoding)` if a `%` is not followed by exactly
/// two hexadecimal digits, or if the resulting byte sequence is not valid UTF‑8.
/// Returns `Err(UrlError::AllocError)` if the output string cannot be allocated.
pub fn url_decode(input: &str) -> Result<String, UrlError> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    if output.capacity() < bytes.len() {
        return Err(UrlError::AllocError);
    }

    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(UrlError::InvalidEncoding);
            }
            let high = bytes[i + 1];
            let low = bytes[i + 2];
            let hex_byte = (hex_digit_value(high)? << 4) | hex_digit_value(low)?;
            output.push(hex_byte);
            i += 3;
        } else {
            output.push(bytes[i]);
            i += 1;
        }
    }

    String::from_utf8(output).map_err(|_| UrlError::InvalidEncoding)
}

/// Converts an ASCII hex digit (0‑9, A‑F, a‑f) to its 4‑bit value.
fn hex_digit_value(d: u8) -> Result<u8, UrlError> {
    match d {
        b'0'..=b'9' => Ok(d - b'0'),
        b'A'..=b'F' => Ok(d - b'A' + 10),
        b'a'..=b'f' => Ok(d - b'a' + 10),
        _ => Err(UrlError::InvalidEncoding),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode() {
        let encoded = url_encode("hello world!").unwrap();
        assert_eq!(encoded, "hello%20world%21");
    }

    #[test]
    fn test_decode() {
        let decoded = url_decode("hello%20world%21").unwrap();
        assert_eq!(decoded, "hello world!");
    }

    #[test]
    fn test_roundtrip() {
        let original = "https://example.com/path?name=value&foo=bar#anchor";
        let encoded = url_encode(original).unwrap();
        let decoded = url_decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }
}
