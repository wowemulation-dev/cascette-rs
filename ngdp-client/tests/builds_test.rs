//! Integration tests for the products builds command

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_builds_help() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.args(["products", "builds", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show all historical builds"))
        .stdout(predicate::str::contains("--filter"))
        .stdout(predicate::str::contains("--days"))
        .stdout(predicate::str::contains("--limit"))
        .stdout(predicate::str::contains("--bgdl-only"));
}

#[test]
fn test_builds_invalid_product() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.args(["products", "builds", "nonexistent"])
        .assert()
        .success() // API call succeeds but returns empty results
        .stdout(predicate::str::contains("No builds found"));
}

#[test]
fn test_builds_json_format() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.args(["products", "builds", "nonexistent", "--format", "json"])
        .assert()
        .success()
        .stdout("[]\n");
}

#[test]
fn test_builds_bpsv_format() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.args(["products", "builds", "nonexistent", "--format", "bpsv"])
        .assert()
        .success()
        .stdout(predicate::str::contains("## seqn = 1"))
        .stdout(predicate::str::contains(
            "product!STRING:0|version!STRING:0|created_at!STRING:0|build_config!HEX:16|is_bgdl!BOOL:0"
        ));
}

// Note: We can't test actual product data as it requires network access and the data changes
// These tests focus on command structure and error handling
