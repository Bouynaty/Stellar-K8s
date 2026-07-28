// tests/backup_restore_smoke_test.rs
// Command-level smoke tests for backup and restore CLI commands.
// These tests validate end-to-end behavior using assert-cmd.
// Related: #1149 - Add command-level smoke tests for backup and restore workflows

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_backup_help_exits_successfully() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["backup", "--help"])
        .assert()
        .success();
}

#[test]
fn test_restore_help_exits_successfully() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["backup", "restore", "--help"])
        .assert()
        .success();
}

#[test]
fn test_backup_list_help_exits_successfully() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["backup", "list", "--help"])
        .assert()
        .success();
}

#[test]
fn test_restore_help_documents_destination() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["backup", "restore", "--help"])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("--destination").or(predicates::str::contains("DESTINATION")),
        );
}

#[test]
fn test_backup_create_help_exits_successfully() {
    Command::cargo_bin("stellar-operator")
        .unwrap()
        .args(["backup", "create", "--help"])
        .assert()
        .success();
}
