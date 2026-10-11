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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().map(String::as_str) else {
        print!("{USAGE}");
        return ExitCode::from(2);
    };

    match cmd {
        "list" => cmd_list(),
        "get" => with_iface(&args, cmd_get),
        "set" => with_mac(&args, cmd_set),
        "random" => with_iface(&args, cmd_random),
        "reset" => with_iface(&args, cmd_reset),
        "original" => with_iface(&args, cmd_original),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("macabre: unknown command {other:?}\n");
            print!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn with_iface(args: &[String], f: fn(&str) -> ExitCode) -> ExitCode {
    match args.get(1) {
        Some(name) => f(name),
        None => {
            eprintln!("macabre: missing interface name");
            ExitCode::from(2)
        }
    }
}

fn with_mac(args: &[String], f: fn(&str, MacAddress) -> ExitCode) -> ExitCode {
    let Some(name) = args.get(1) else {
        eprintln!("macabre: missing interface name");
        return ExitCode::from(2);
    };
    let Some(mac_str) = args.get(2) else {
        eprintln!("macabre: missing MAC address");
        return ExitCode::from(2);
    };
    match mac_str.parse::<MacAddress>() {
        Ok(mac) => f(name, mac),
        Err(e) => {
            eprintln!("macabre: bad MAC {mac_str:?}: {e}");
            ExitCode::from(2)
        }
    }
}

