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
    help                 show this help
    version              show the version

MAC format: six hex octets separated by colons, e.g. 02:ab:cd:ef:00:11
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().map(String::as_str) else {
        print!("{USAGE}");
        return ExitCode::from(2);
    };

    match cmd {
        "list" if args.len() == 1 => cmd_list(),
        "list" => {
            eprintln!("macabre: 'list' takes no arguments");
            ExitCode::from(2)
        }
        "get" => with_iface(&args, cmd_get),
        "set" => with_mac(&args, cmd_set),
        "random" => with_iface(&args, cmd_random),
        "reset" => with_iface(&args, cmd_reset),
        "original" => with_iface(&args, cmd_original),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        "-V" | "--version" | "version" => {
            println!("macabre {}", env!("CARGO_PKG_VERSION"));
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
    if args.len() != 2 {
        eprintln!("macabre: expected exactly one interface name");
        return ExitCode::from(2);
    }
    match args.get(1) {
        Some(name) => f(name),
        None => {
            eprintln!("macabre: missing interface name");
            ExitCode::from(2)
        }
    }
}

fn with_mac(args: &[String], f: fn(&str, MacAddress) -> ExitCode) -> ExitCode {
    if args.len() != 3 {
        eprintln!("macabre: expected an interface name and one MAC address");
        return ExitCode::from(2);
    }
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
    if !mac.is_unicast() {
        return fail("refusing multicast MAC — that would confuse the network");
    }
    if !mac.is_locally_administered() {
        eprintln!("macabre: note: {mac} is not locally administered");
    }
    // Capture the original before the change, but only persist it once the
    // change succeeded: a failed set must not leave a store entry claiming
    // an "original" that is really just the unchanged address.
    let original = match iface::get_mac(name) {
        Ok(current) => current,
        Err(e) => return fail(&format!("{name}: {e}")),
    };
    if let Err(e) = iface::set_mac(name, mac) {
        return fail(&format!("{name}: {e}"));
    }
    if let Err(e) = iface::save_original(name, original) {
        // Undo the change so the user is never stuck with a new MAC and no
        // way back through `reset`.
        let _ = iface::set_mac(name, original);
        return fail(&format!("saving original MAC: {e}"));
    }
    println!("{name}: MAC set to {mac}");
    ExitCode::SUCCESS
}

fn cmd_random(name: &str) -> ExitCode {
    let mac = match MacAddress::random_local() {
        Ok(mac) => mac,
        Err(e) => return fail(&format!("generating a random MAC: {e}")),
    };
    cmd_set(name, mac)
}

fn cmd_reset(name: &str) -> ExitCode {
    match iface::load_original(name) {
        Err(e) => fail(&format!("reading saved MAC: {e}")),
        Ok(None) => fail(&format!("{name}: no saved original MAC — nothing to reset")),
        Ok(Some(mac)) => {
            // The store validates hex syntax, not safety; a tampered entry
            // must not let us put a multicast address on the wire.
            if !mac.is_unicast() {
                return fail(&format!(
                    "{name}: refusing multicast saved MAC {mac} — store entry is corrupt"
                ));
            }
            match iface::set_mac(name, mac) {
                Ok(()) => match iface::clear_original(name) {
                    Ok(()) => {
                        println!("{name}: restored to {mac}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => fail(&format!("restored, but clearing the store failed: {e}")),
                },
                Err(e) => fail(&format!("{name}: {e}")),
            }
        }
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

fn fail(msg: &str) -> ExitCode {
    eprintln!("macabre: {msg}");
    ExitCode::FAILURE
}
