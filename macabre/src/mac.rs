//! MAC address type: parsing, formatting, validation, generation.

use std::fmt;
use std::str::FromStr;

/// A 48-bit IEEE 802 MAC address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacAddress([u8; 6]);

