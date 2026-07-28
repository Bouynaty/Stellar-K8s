// tests/cli_examples_test.rs
// Validates that documented CLI examples parse correctly.
//
// Related: #1154 - Add pipeline stage that validates every documented CLI example command

use assert_cmd::Command;

#[test]
fn test_cli_help_examples() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn test_cli_run_command_accepted() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["run", "--help"])
        .assert()
        .success();
}

#[test]
fn test_cli_webhook_command_accepted() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["webhook", "--help"])
        .assert()
        .success();
}

#[test]
fn test_cli_info_command_accepted() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["info", "--help"])
        .assert()
        .success();
}

#[test]
fn test_kubectl_stellar_list_command() {
    Command::cargo_bin("kubectl-stellar")
        .unwrap()
        .args(["list", "--help"])
        .assert()
        .success();
}

#[test]
fn test_stellar_operator_benchmark_command() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["benchmark", "--help"])
        .assert()
        .success();
}

#[test]
fn test_backup_subcommand() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["backup", "--help"])
        .assert()
        .success();
}

#[test]
fn test_backup_restore_subcommand() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["backup", "restore", "--help"])
        .assert()
        .success();
}

#[test]
fn test_simulator_subcommand() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["simulator", "--help"])
        .assert()
        .success();
}

#[test]
fn test_completions_subcommand() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["completions", "--help"])
        .assert()
        .success();
}
