//! Integration tests for certificate commands

use assert_cmd::Command;
use ngdp_client::test_constants::EXAMPLE_CERT_HASH;
use predicates::prelude::*;

#[test]
fn test_certs_download_help() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.arg("certs")
        .arg("download")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Download a certificate by its SKI/hash",
        ))
        .stdout(predicate::str::contains("--cert-format"))
        .stdout(predicate::str::contains("--details"));
}

#[test]
fn test_certs_download_missing_ski() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.arg("certs")
        .arg("download")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "required arguments were not provided",
        ));
}

#[test]
fn test_certs_download_invalid_region() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.arg("certs")
        .arg("download")
        .arg("test-ski")
        .arg("--region")
        .arg("invalid")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid region"));
}

#[test]
fn test_certs_download_invalid_format() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.arg("certs")
        .arg("download")
        .arg("test-ski")
        .arg("--cert-format")
        .arg("invalid")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "invalid value 'invalid' for '--cert-format",
        ));
}

#[test]
fn test_certs_download_with_json_output() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.arg("certs")
        .arg("download")
        .arg(EXAMPLE_CERT_HASH)
        .arg("--details")
        .arg("-o")
        .arg("json")
        .arg("--no-cache") // Skip cache for testing
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"certificate\""))
        .stdout(predicate::str::contains("\"ski\""))
        .stdout(predicate::str::contains("\"details\""));
}

#[test]
fn test_certs_subcommand_exists() {
    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.arg("certs")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Certificate operations"))
        .stdout(predicate::str::contains("download"));
}

#[test]
fn test_certs_download_output_file() {
    use std::fs;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let output_path = dir.path().join("test-cert.pem");

    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.arg("certs")
        .arg("download")
        .arg(EXAMPLE_CERT_HASH)
        .arg("--output")
        .arg(&output_path)
        .arg("--no-cache")
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success();

    // Verify file was created
    assert!(output_path.exists());
    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("BEGIN CERTIFICATE"));
    assert!(content.contains("END CERTIFICATE"));
}

#[test]
fn test_certs_download_der_format() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let output_path = dir.path().join("test-cert.der");

    let mut cmd = Command::cargo_bin("ngdp").unwrap();
    cmd.arg("certs")
        .arg("download")
        .arg(EXAMPLE_CERT_HASH)
        .arg("--output")
        .arg(&output_path)
        .arg("--cert-format")
        .arg("der")
        .arg("--no-cache")
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success();

    // Verify file was created and is binary
    assert!(output_path.exists());
    let content = std::fs::read(&output_path).unwrap();
    // DER format should start with 0x30 (SEQUENCE tag)
    assert_eq!(content[0], 0x30);
}
