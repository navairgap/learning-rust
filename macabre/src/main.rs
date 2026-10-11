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

fn cmd_list() -> ExitCode {
    match iface::list_interfaces() {
        Ok(list) => {
            if list.is_empty() {
                println!("no interfaces found");
            }
            for i in list {
                println!("{:<16} {}", i.name, i.mac);
            }
            ExitCode::SUCCESS
        }
        Err(e) => fail(&format!("listing interfaces: {e}")),
    }
}

fn cmd_get(name: &str) -> ExitCode {
    match iface::get_mac(name) {
        Ok(mac) => {
            println!("{mac}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(&format!("{name}: {e}")),
    }
}

fn cmd_set(name: &str, mac: MacAddress) -> ExitCode {
    if !root() {
        return fail("setting a MAC requires root (run with sudo)");
    }
    if let Err(e) = original_guard(name) {
        return fail(&format!("saving original MAC: {e}"));
    }
    match iface::set_mac(name, mac) {
        Ok(()) => {
            println!("{name}: MAC set to {mac}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(&format!("{name}: {e}")),
    }
}

fn cmd_random(name: &str) -> ExitCode {
    let mac = MacAddress::random_local();
    println!("{name}: using {mac}");
    cmd_set(name, mac)
}

fn cmd_reset(name: &str) -> ExitCode {
    if !root() {
        return fail("resetting a MAC requires root (run with sudo)");
    }
    match iface::load_original(name) {
        Err(e) => fail(&format!("reading saved MAC: {e}")),
        Ok(None) => fail(&format!("{name}: no saved original MAC — nothing to reset")),
        Ok(Some(mac)) => match iface::set_mac(name, mac) {
            Ok(()) => match iface::clear_original(name) {
                Ok(()) => {
                    println!("{name}: restored to {mac}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(&format!("restored, but clearing the store failed: {e}")),
            },
            Err(e) => fail(&format!("{name}: {e}")),
        },
    }
}

fn cmd_original(name: &str) -> ExitCode {
    match iface::load_original(name) {
        Ok(Some(mac)) => {
            println!("{mac}");
            ExitCode::SUCCESS
        }
        Ok(None) => {
            eprintln!("{name}: no saved original MAC");
            ExitCode::from(1)
        }
        Err(e) => fail(&format!("reading store: {e}")),
    }
}

/// Save the interface's current MAC as "the original" the first time we touch it.
fn original_guard(name: &str) -> std::io::Result<()> {
    match iface::load_original(name)? {
        Some(_) => Ok(()),
        None => {
            let current = iface::get_mac(name)?;
            iface::save_original(name, current)
        }
    }
}

fn root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("macabre: {msg}");
    ExitCode::FAILURE
}
