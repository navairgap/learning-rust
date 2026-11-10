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
fn version_flag_prints_the_crate_version() {
    let expected = format!("macabre {}", env!("CARGO_PKG_VERSION"));

    for args in [
        vec!["--version"],
        vec!["-V"],
        vec!["version"],
    ] {
        let output = macabre().args(args).output().expect("run macabre");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.starts_with(expected.as_str()),
            "unexpected version output: {stdout}"
        );
    }
}

#[test]
fn unknown_command_returns_usage_error() {
    let output = macabre()
        .arg("not-a-command")
        .output()
        .expect("run macabre");

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
    let output = macabre()
        .args(["set", "wlan0"])
        .output()
        .expect("run macabre");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("interface name and one MAC"));
}

#[test]
fn invalid_mac_is_rejected_before_interface_or_privilege_checks() {
    let output = macabre()
        .args(["set", "missing-interface", "not-a-mac"])
        .output()
        .expect("run macabre");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bad MAC"), "unexpected stderr: {stderr}");
    assert!(
        !stderr.contains("requires root"),
        "MAC parsing must happen first"
    );
}

#[test]
fn random_does_not_print_a_mac_when_interface_validation_fails() {
    let output = macabre()
        .args(["random", ""])
        .output()
        .expect("run macabre");

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "must not announce an unapplied address"
    );
}

#[test]
fn multicast_mac_is_rejected_before_interface_operations() {
    let output = macabre()
        .args(["set", "missing-interface-for-test", "01:00:00:00:00:01"])
        .output()
        .expect("run macabre");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("refusing multicast MAC"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        !stderr.contains("missing-interface"),
        "interface lookup should not run"
    );
}

#[test]
fn valid_mac_attempt_is_not_rejected_by_an_euid_only_root_gate() {
    let output = macabre()
        .args(["set", "missing-interface-for-test", "02:00:00:00:00:01"])
        .output()
        .expect("run macabre");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("requires root"),
        "capability-aware ioctl path should decide access"
    );
}
