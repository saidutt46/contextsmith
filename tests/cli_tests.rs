use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn cmd() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("contextsmith").unwrap()
}

#[test]
fn help_shows_all_commands() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("diff"))
        .stdout(predicate::str::contains("collect"))
        .stdout(predicate::str::contains("pack"))
        .stdout(predicate::str::contains("trim"))
        .stdout(predicate::str::contains("map"))
        .stdout(predicate::str::contains("stats"))
        .stdout(predicate::str::contains("explain"));
}

#[test]
fn init_creates_config_and_cache() {
    let dir = tempdir().unwrap();
    cmd()
        .args(["init", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created config"));

    assert!(dir.path().join("contextsmith.toml").exists());
    assert!(dir.path().join(".contextsmith/cache").exists());
}

#[test]
fn init_no_cache_skips_cache_dir() {
    let dir = tempdir().unwrap();
    cmd()
        .args(["init", "--root", dir.path().to_str().unwrap(), "--no-cache"])
        .assert()
        .success();

    assert!(dir.path().join("contextsmith.toml").exists());
    assert!(!dir.path().join(".contextsmith").exists());
}

#[test]
fn init_errors_on_existing_without_force() {
    let dir = tempdir().unwrap();

    // First init
    cmd()
        .args(["init", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success();

    // Second init should fail
    cmd()
        .args(["init", "--root", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn init_force_overwrites() {
    let dir = tempdir().unwrap();

    cmd()
        .args(["init", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success();

    cmd()
        .args(["init", "--root", dir.path().to_str().unwrap(), "--force"])
        .assert()
        .success();
}

#[test]
fn unimplemented_command_shows_error() {
    cmd()
        .arg("diff")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not yet implemented"));
}
