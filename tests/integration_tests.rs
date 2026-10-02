use devsweep::cleaner::validate_artifact_safety;
use devsweep::scanner::model::{format_bytes, Ecosystem};
use devsweep::scanner::rules::{is_known_artifact_name, match_ecosystems};
use std::fs::{self, File};
use tempfile::tempdir;

#[test]
fn test_format_bytes() {
    assert_eq!(format_bytes(500), "500 B");
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    assert_eq!(format_bytes(1024 * 1024 * 1024 * 3), "3.00 GB");
}

#[test]
fn test_is_known_artifact_name() {
    assert!(is_known_artifact_name("target"));
    assert!(is_known_artifact_name("node_modules"));
    assert!(is_known_artifact_name(".venv"));
    assert!(is_known_artifact_name("build"));
    assert!(!is_known_artifact_name("src"));
    assert!(!is_known_artifact_name("Cargo.toml"));
}

#[test]
fn test_match_ecosystems_rust() {
    let dir = tempdir().unwrap();
    let cargo_toml = dir.path().join("Cargo.toml");
    File::create(&cargo_toml).unwrap();

    let matches = match_ecosystems(dir.path());
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].ecosystem, Ecosystem::Rust);
}

#[test]
fn test_match_ecosystems_node() {
    let dir = tempdir().unwrap();
    let pkg_json = dir.path().join("package.json");
    File::create(&pkg_json).unwrap();

    let matches = match_ecosystems(dir.path());
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].ecosystem, Ecosystem::Node);
}

#[test]
fn test_validate_artifact_safety() {
    let dir = tempdir().unwrap();
    let target_dir = dir.path().join("target");
    fs::create_dir(&target_dir).unwrap();

    // Valid artifact folder
    assert!(validate_artifact_safety(dir.path(), &target_dir).is_ok());

    // Root folder cannot be deleted
    assert!(validate_artifact_safety(dir.path(), dir.path()).is_err());

    // Non-artifact folder cannot be deleted
    let src_dir = dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();
    assert!(validate_artifact_safety(dir.path(), &src_dir).is_err());
}
