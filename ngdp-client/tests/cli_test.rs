//! Integration tests for the ngdp CLI

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_command() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "NGDP (Next Generation Distribution Pipeline)",
        ))
        .stdout(predicate::str::contains("Commands:"))
        .stdout(predicate::str::contains("products"))
        .stdout(predicate::str::contains("storage"))
        .stdout(predicate::str::contains("download"))
        .stdout(predicate::str::contains("inspect"))
        .stdout(predicate::str::contains("config"));
}

#[test]
fn test_version_command() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("ngdp"));
}

#[test]
fn test_products_help() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.args(["products", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Query product information"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("versions"))
        .stdout(predicate::str::contains("cdns"))
        .stdout(predicate::str::contains("info"));
}

#[test]
fn test_invalid_command() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.arg("invalid")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn test_output_format_options() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.args(["--format", "json", "config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r#"\{.*\}"#).unwrap());
}

#[test]
fn test_config_get_command() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.args(["config", "get", "default_region"])
        .assert()
        .success()
        .stdout(predicate::str::contains("us"));
}

#[test]
fn test_config_get_nonexistent() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.args(["config", "get", "nonexistent_key"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_config_reset_requires_confirmation() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.args(["config", "reset"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires confirmation"));
}
