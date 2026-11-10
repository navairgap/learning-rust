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
fn selftest_passes_unprivileged() {
    let output = macabre().arg("selftest").output().expect("run macabre");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for section in ["entropy", "interfaces", "store"] {
        assert!(
            stdout.contains(section),
            "missing selftest section: {section}"
        );
    }
    assert!(stdout.contains("selftest passed"));
}

#[test]
fn version_flag_prints_the_crate_version() {
    let expected = format!("macabre {}", env!("CARGO_PKG_VERSION"));

    for args in [vec!["--version"], vec!["-V"], vec!["version"]] {
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
fn no_arguments_prints_usage_to_stdout_and_exits_2() {
    let output = macabre().output().expect("run macabre");

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("USAGE:"),
        "usage goes to stdout for a tty user"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn help_subcommand_is_also_successful() {
    let output = macabre().arg("help").output().expect("run macabre");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("USAGE:"));
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

/// An isolated XDG_DATA_HOME so store-reading tests never touch real user data.
fn isolated_data_home() -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("macabre-cli-test-{unique}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn original_reports_missing_entry_with_exit_1() {
    let output = macabre()
        .env("XDG_DATA_HOME", isolated_data_home())
        .args(["original", "wlan0"])
        .output()
        .expect("run macabre");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no saved original MAC"),
        "unexpected stderr: {stderr}"
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn reset_without_a_saved_mac_explains_and_exits_1() {
    let output = macabre()
        .env("XDG_DATA_HOME", isolated_data_home())
        .args(["reset", "wlan0"])
        .output()
        .expect("run macabre");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nothing to reset"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn get_on_a_missing_interface_exits_1_with_the_os_error() {
    let output = macabre()
        .args(["get", "macabre-no-such-iface"])
        .output()
        .expect("run macabre");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("macabre-no-such-iface"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        !stderr.contains("refusing"),
        "a valid unicast attempt must reach the ioctl path"
    );
}

#[test]
fn list_runs_unprivileged_and_exits_zero() {
    let output = macabre().arg("list").output().expect("run macabre");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn selftest_rejects_extra_arguments() {
    let output = macabre()
        .args(["selftest", "extra"])
        .output()
        .expect("run macabre");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("takes no arguments"));
}
