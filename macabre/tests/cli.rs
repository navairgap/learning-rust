//! CLI contract tests. These cases validate arguments only and never
//! invoke commands that modify a network interface.

use std::process::Command;

fn macabre() -> Command {
    Command::new(env!("CARGO_BIN_EXE_macabre"))
}

#[test]
fn help_is_successful_and_describes_commands() {
    let output = macabre().arg("--help").output().expect("run macabre");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("USAGE:"));
    assert!(stdout.contains("reset <iface>"));
}

#[test]
fn unknown_command_returns_usage_error() {
    let output = macabre().arg("not-a-command").output().expect("run macabre");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown command"));
}

#[test]
fn interface_commands_require_exactly_one_argument() {
    for args in [vec!["get"], vec!["get", "wlan0", "extra"], vec!["reset"]] {
        let output = macabre().args(args).output().expect("run macabre");
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("exactly one interface"));
    }
}

#[test]
fn set_requires_both_interface_and_mac_before_system_operations() {
    let output = macabre().args(["set", "wlan0"]).output().expect("run macabre");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("interface name and one MAC"));
}
