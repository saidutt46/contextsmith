use assert_cmd::Command;
use predicates::prelude::*;
use std::process;
use tempfile::tempdir;

fn cmd() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("contextsmith").unwrap()
}

/// Helper: create a temporary git repo with an initial commit and a
/// subsequent modification, returning the tempdir handle and root path.
fn setup_git_repo() -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Initialise a git repo with a deterministic author.
    git(root, &["init"]);
    git(root, &["config", "user.email", "test@test.com"]);
    git(root, &["config", "user.name", "Test"]);

    // Create initial file and commit.
    std::fs::write(
        root.join("hello.rs"),
        "fn main() {\n    println!(\"hello\");\n}\n",
    )
    .unwrap();
    git(root, &["add", "hello.rs"]);
    git(root, &["commit", "-m", "initial"]);

    // Modify the file to produce a diff.
    std::fs::write(
        root.join("hello.rs"),
        "fn main() {\n    println!(\"hello, world!\");\n    println!(\"welcome\");\n}\n",
    )
    .unwrap();

    dir
}

/// Run a git command in the given directory, panicking on failure.
fn git(dir: &std::path::Path, args: &[&str]) {
    let status = process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdout(process::Stdio::null())
        .stderr(process::Stdio::null())
        .status()
        .expect("git command failed to start");
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

// -----------------------------------------------------------------------
// General CLI tests
// -----------------------------------------------------------------------

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
fn unimplemented_command_shows_error() {
    // `collect` is still stubbed — verify it reports not-implemented.
    cmd()
        .arg("collect")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not yet implemented"));
}

// -----------------------------------------------------------------------
// Init command tests
// -----------------------------------------------------------------------

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
    cmd()
        .args(["init", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success();

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

// -----------------------------------------------------------------------
// Diff command tests
// -----------------------------------------------------------------------

#[test]
fn diff_shows_changes_in_markdown() {
    let dir = setup_git_repo();
    cmd()
        .args(["diff", "--root", dir.path().to_str().unwrap(), "--stdout"])
        .assert()
        .success()
        .stdout(predicate::str::contains("## `hello.rs`"))
        .stdout(predicate::str::contains("hello, world!"));
}

#[test]
fn diff_staged_only() {
    let dir = setup_git_repo();
    let root = dir.path();

    // Stage the changes.
    git(root, &["add", "hello.rs"]);

    cmd()
        .args([
            "diff",
            "--root",
            root.to_str().unwrap(),
            "--staged",
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello, world!"));
}

#[test]
fn diff_json_format_is_valid() {
    let dir = setup_git_repo();
    let output = cmd()
        .args([
            "diff",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--stdout",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(parsed["sections"].is_array());
}

#[test]
fn diff_hunks_only_produces_diff_markers() {
    let dir = setup_git_repo();
    cmd()
        .args([
            "diff",
            "--root",
            dir.path().to_str().unwrap(),
            "--hunks-only",
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("+"));
}

#[test]
fn diff_no_changes_shows_message() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    git(root, &["init"]);
    git(root, &["config", "user.email", "test@test.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::write(root.join("file.txt"), "content\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "init"]);

    // No modifications — should report no changes.
    cmd()
        .args(["diff", "--root", root.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("No changes"));
}

#[test]
fn diff_non_git_directory_errors() {
    let dir = tempdir().unwrap();
    cmd()
        .args(["diff", "--root", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("git"));
}

#[test]
fn diff_rev_range() {
    let dir = setup_git_repo();
    let root = dir.path();

    // Commit the modification so we can diff HEAD~1..HEAD.
    git(root, &["add", "hello.rs"]);
    git(root, &["commit", "-m", "update"]);

    cmd()
        .args([
            "diff",
            "--root",
            root.to_str().unwrap(),
            "HEAD~1..HEAD",
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello, world!"));
}

#[test]
fn diff_output_to_file() {
    let dir = setup_git_repo();
    let out_file = dir.path().join("output.md");

    cmd()
        .args([
            "diff",
            "--root",
            dir.path().to_str().unwrap(),
            "--out",
            out_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&out_file).unwrap();
    assert!(content.contains("hello, world!"));
}
