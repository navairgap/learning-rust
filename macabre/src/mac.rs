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

impl std::error::Error for ParseMacError {}

impl MacAddress {
    /// Build from raw octets.
    pub fn from_octets(octets: [u8; 6]) -> Self {
        MacAddress(octets)
    }

    /// Raw octets, big-endian wire order.
    pub fn octets(&self) -> [u8; 6] {
        self.0
    }

    /// Second-least-significant bit of the first octet: 1 means locally administered.
    pub fn is_locally_administered(&self) -> bool {
        self.0[0] & 0x02 != 0
    }

    /// Least-significant bit of the first octet: 0 means unicast, 1 means multicast.
    pub fn is_unicast(&self) -> bool {
        self.0[0] & 0x01 == 0
    }

