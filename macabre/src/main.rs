//! macabre — your MAC, requiem'd.
//!
//! Change and restore network interface MAC addresses on Linux.
//!
//! Usage: macabre <command> [args]

mod iface;
mod mac;

use std::process::ExitCode;

use mac::MacAddress;

const USAGE: &str = "macabre — your MAC, requiem'd

USAGE:
    macabre <command> [arguments]

COMMANDS:
    list                 show all interfaces and their MAC addresses
    get <iface>          show the current MAC of an interface
    set <iface> <mac>    set a specific MAC address (root required)
    random <iface>       set a random locally-administered MAC (root required)
    reset <iface>        restore the saved original MAC (root required)
    original <iface>     show the saved original MAC, if any

MAC format: six hex octets separated by colons, e.g. 02:ab:cd:ef:00:11
";

