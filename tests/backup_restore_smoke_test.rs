//! Command-level smoke tests for backup and restore workflows.
//!
//! These tests validate that the backup and restore CLI commands work correctly
//! end-to-end with the file backend. This ensures the core backup/restore workflows
//! are functional and catches regressions.
//!
//! Related: #1149 - Add command-level smoke tests for backup and restore workflows

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to build the CLI command
fn stellar_operator() -> Command {
    Command::cargo_bin("stellar-operator").expect("Binary should exist after cargo build")
}

#[test]
fn backup_create_file_backend_creates_tarball() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_dir = temp_dir.path().join("source");
    let dest_dir = temp_dir.path().join("backups");
    fs::create_dir_all(&source_dir).expect("Failed to create source dir");
    fs::create_dir_all(&dest_dir).expect("Failed to create dest dir");

    // Create a test file to backup
    fs::write(source_dir.join("test.txt"), "Hello, Stellar!").expect("Failed to write test file");

    let mut cmd = stellar_operator();
    cmd.args([
        "backup",
        "create",
        "--source",
        source_dir.to_str().unwrap(),
        "--backend",
        "file",
        "--destination",
        dest_dir.to_str().unwrap(),
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Backup completed"))
        .stdout(predicate::str::contains("Backup created at"));

    // Verify a .tar.gz file was created
    let backups: Vec<_> = fs::read_dir(&dest_dir)
        .expect("Failed to read dest dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "tar.gz").unwrap_or(false))
        .collect();

    assert_eq!(backups.len(), 1, "Expected exactly one .tar.gz backup file");
    assert!(
        backups[0]
            .file_name()
            .to_string_lossy()
            .starts_with("backup-"),
        "Backup file should start with 'backup-' prefix"
    );
}

#[test]
fn backup_restore_roundtrip_preserves_data() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_dir = temp_dir.path().join("source");
    let backup_dir = temp_dir.path().join("backups");
    let restore_dir = temp_dir.path().join("restore");
    fs::create_dir_all(&source_dir).expect("Failed to create source dir");
    fs::create_dir_all(&backup_dir).expect("Failed to create backup dir");

    // Create test data structure
    fs::write(source_dir.join("file1.txt"), "Content 1").expect("Failed to write file1");
    fs::create_dir_all(source_dir.join("subdir")).expect("Failed to create subdir");
    fs::write(
        source_dir.join("subdir").join("file2.txt"),
        "Content 2",
    )
    .expect("Failed to write file2");

    // Step 1: Create backup
    let mut backup_cmd = stellar_operator();
    backup_cmd.args([
        "backup",
        "create",
        "--source",
        source_dir.to_str().unwrap(),
        "--backend",
        "file",
        "--destination",
        backup_dir.to_str().unwrap(),
    ]);

    backup_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup completed"));

    // Find the backup file
    let backup_file = fs::read_dir(&backup_dir)
        .expect("Failed to read backup dir")
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().map(|ext| ext == "tar.gz").unwrap_or(false))
        .expect("No backup file found");

    // Step 2: Restore backup
    let mut restore_cmd = stellar_operator();
    restore_cmd.args([
        "backup",
        "restore",
        "--backup",
        backup_file.path().to_str().unwrap(),
        "--destination",
        restore_dir.to_str().unwrap(),
        "--backend",
        "file",
    ]);

    restore_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Restore completed"));

    // Step 3: Verify restored data matches original
    assert!(
        restore_dir.join("file1.txt").exists(),
        "file1.txt should exist in restored backup"
    );
    assert_eq!(
        fs::read_to_string(restore_dir.join("file1.txt")).expect("Failed to read restored file1"),
        "Content 1"
    );

    assert!(
        restore_dir.join("subdir").join("file2.txt").exists(),
        "subdir/file2.txt should exist in restored backup"
    );
    assert_eq!(
        fs::read_to_string(restore_dir.join("subdir").join("file2.txt"))
            .expect("Failed to read restored file2"),
        "Content 2"
    );
}

#[test]
fn backup_list_shows_existing_backups() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let backup_dir = temp_dir.path().join("backups");
    fs::create_dir_all(&backup_dir).expect("Failed to create backup dir");

    // Create two backup files manually
    fs::write(backup_dir.join("backup-20240101-120000.tar.gz"), "fake backup 1")
        .expect("Failed to write fake backup 1");
    fs::write(backup_dir.join("backup-20240102-120000.tar.gz"), "fake backup 2")
        .expect("Failed to write fake backup 2");

    let mut cmd = stellar_operator();
    cmd.args([
        "backup",
        "list",
        "--location",
        backup_dir.to_str().unwrap(),
        "--backend",
        "file",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Found 2 backups"))
        .stdout(predicate::str::contains("backup-20240101-120000.tar.gz"))
        .stdout(predicate::str::contains("backup-20240102-120000.tar.gz"));
}

#[test]
fn backup_cleanup_removes_old_backups() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let backup_dir = temp_dir.path().join("backups");
    fs::create_dir_all(&backup_dir).expect("Failed to create backup dir");

    // Create three backup files with different timestamps
    let backups = vec![
        ("backup-20240101-120000.tar.gz", 0), // oldest
        ("backup-20240102-120000.tar.gz", 1),
        ("backup-20240103-120000.tar.gz", 2), // newest
    ];

    for (name, _) in &backups {
        fs::write(backup_dir.join(name), "fake backup").expect("Failed to write fake backup");
    }

    // Verify we have 3 backups
    assert_eq!(
        fs::read_dir(&backup_dir).unwrap().count(),
        3,
        "Should have 3 backups initially"
    );

    // Run cleanup with --keep=1 (should keep only the newest)
    let mut cmd = stellar_operator();
    cmd.args([
        "backup",
        "cleanup",
        "--location",
        backup_dir.to_str().unwrap(),
        "--backend",
        "file",
        "--keep",
        "1",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Deleted 2 old backups"));

    // Verify only 1 backup remains
    let remaining: Vec<_> = fs::read_dir(&backup_dir)
        .expect("Failed to read backup dir")
        .filter_map(|e| e.ok())
        .collect();

    assert_eq!(remaining.len(), 1, "Should have 1 backup after cleanup");
    assert_eq!(
        remaining[0].file_name().to_string_lossy(),
        "backup-20240103-120000.tar.gz",
        "Should keep the newest backup"
    );
}

#[test]
fn backup_cleanup_does_nothing_when_fewer_than_keep() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let backup_dir = temp_dir.path().join("backups");
    fs::create_dir_all(&backup_dir).expect("Failed to create backup dir");

    // Create only 2 backup files
    fs::write(backup_dir.join("backup-20240101.tar.gz"), "backup 1")
        .expect("Failed to write backup 1");
    fs::write(backup_dir.join("backup-20240102.tar.gz"), "backup 2")
        .expect("Failed to write backup 2");

    // Run cleanup with --keep=5 (more than we have)
    let mut cmd = stellar_operator();
    cmd.args([
        "backup",
        "cleanup",
        "--location",
        backup_dir.to_str().unwrap(),
        "--backend",
        "file",
        "--keep",
        "5",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No backups to delete"));

    // Verify both backups still exist
    assert_eq!(
        fs::read_dir(&backup_dir).unwrap().count(),
        2,
        "Should still have 2 backups"
    );
}

#[test]
fn backup_create_rejects_nonexistent_source() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dest_dir = temp_dir.path().join("backups");
    fs::create_dir_all(&dest_dir).expect("Failed to create dest dir");

    let mut cmd = stellar_operator();
    cmd.args([
        "backup",
        "create",
        "--source",
        "/nonexistent/path",
        "--backend",
        "file",
        "--destination",
        dest_dir.to_str().unwrap(),
    ]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "path does not exist",
    ));
}

#[test]
fn backup_restore_rejects_nonexistent_backup() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let restore_dir = temp_dir.path().join("restore");
    fs::create_dir_all(&restore_dir).expect("Failed to create restore dir");

    let mut cmd = stellar_operator();
    cmd.args([
        "backup",
        "restore",
        "--backup",
        "/nonexistent/backup.tar.gz",
        "--destination",
        restore_dir.to_str().unwrap(),
        "--backend",
        "file",
    ]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("backup file not found"));
}

#[test]
fn backup_list_rejects_invalid_location() {
    let mut cmd = stellar_operator();
    cmd.args([
        "backup",
        "list",
        "--location",
        "/nonexistent/directory",
        "--backend",
        "file",
    ]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("location is not a directory"));
}

#[test]
fn backup_commands_reject_unsupported_backend() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_dir = temp_dir.path().join("source");
    fs::create_dir_all(&source_dir).expect("Failed to create source dir");

    // Test backup create with unsupported backend
    let mut cmd = stellar_operator();
    cmd.args([
        "backup",
        "create",
        "--source",
        source_dir.to_str().unwrap(),
        "--backend",
        "unsupported-backend",
        "--destination",
        "/tmp",
    ]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unsupported backend"));

    // Test restore with unsupported backend
    let mut restore_cmd = stellar_operator();
    restore_cmd.args([
        "backup",
        "restore",
        "--backup",
        "fake.tar.gz",
        "--destination",
        "/tmp",
        "--backend",
        "unsupported-backend",
    ]);

    restore_cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported backend"));
}