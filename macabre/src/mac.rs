//! MAC address type: parsing, formatting, validation, generation.

use std::fmt;
use std::str::FromStr;

/// A 48-bit IEEE 802 MAC address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacAddress([u8; 6]);

/// Errors that can occur when parsing a MAC address string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseMacError {
    /// Wrong number of octets (expected exactly 6).
    BadLength,
    /// A non-hex or out-of-range octet.
    BadOctet,
}

impl fmt::Display for ParseMacError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseMacError::BadLength => write!(f, "expected 6 colon-separated octets"),
            ParseMacError::BadOctet => write!(f, "invalid hex octet"),
        }
    }
}

