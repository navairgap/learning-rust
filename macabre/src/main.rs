//! macabre — your MAC, requiem'd.
//!
//! Change and restore network interface MAC addresses on Linux.
//!
//! Usage: macabre <command> [args]

mod iface;
mod mac;

use std::process::ExitCode;

use mac::MacAddress;

