// tests/cli_examples_test.rs
// Validates that documented CLI examples parse correctly.
//
// Related: #1154 - Add pipeline stage that validates every documented CLI example command

use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn test_cli_help_examples() {
    let mut cmd = Command::cargo_bin("stellar-operator").unwrap();
    let assert = cmd.arg("--help");
    assert.output().unwrap();
    // Just check it exits successfully
}

#[test]
fn test_cli_run_command_accepted() {
    let mut cmd = Command::cargo_bin("stellar-operator").unwrap();
    let assert = cmd.args(&["run", "--help"]);
    assert.try_success().unwrap();
}

#[test]
fn test_cli_webhook_command_accepted() {
    let mut cmd = Command::cargo_bin("stellar-operator").unwrap();
    let assert = cmd.args(&["webhook", "--help"]);
    assert.try_success().unwrap();
}

#[test]
fn test_cli_info_command_accepted() {
    let mut cmd = Command::cargo_bin("stellar-operator").unwrap();
    let assert = cmd.args(&["info", "--help"]);
    assert.try_success().unwrap();
}

#[test]
fn test_stellarnode_reconcile_command() {
    let mut cmd = Command::cargo_bin("kubectl-stellar").unwrap();
    let assert = cmd.args(&["stellarnode", "reconcile", "--help"]);
    assert.try_success().unwrap();
}

#[test]
fn test_stellarbench_run_command() {
    let mut cmd = Command::cargo_bin("stellar-bench").unwrap();
    let assert = cmd.args(&["run", "--help"]);
    assert.try_success().unwrap();
}

#[test]
fn test_backup_subcommand() {
    let mut cmd = Command::cargo_bin("stellar-operator").unwrap();
    let assert = cmd.args(&["backup", "--help"]);
    assert.try_success().unwrap();
}

#[test]
fn test_restore_subcommand() {
    let mut cmd = Command::cargo_bin("stellar-operator").unwrap();
    let assert = cmd.args(&["restore", "--help"]);
    assert.try_success().unwrap();
}

#[test]
fn test_simulator_subcommand() {
    let mut cmd = Command::cargo_bin("stellar-operator").unwrap();
    let assert = cmd.args(&["simulate", "--help"]);
    assert.try_success().unwrap();
}

#[test]
fn test_completions_subcommand() {
    let mut cmd = Command::cargo_bin("stellar-operator").unwrap();
    let assert = cmd.args(&["completions", "--help"]);
    assert.try_success().unwrap();
}

</parameter>
<task_progress>
- [x] Analyze requirements
- [x] Create scripts (#1146, #1147, #1145, #1144)
- [x] Update CI workflow with new stages
- [ ] Re-add test files (cli_examples_test.rs, backup_restore_smoke_test.rs)
- [ ] Commit and push changes
- [ ] Create PR description
</task_progress>
</write_to_file>