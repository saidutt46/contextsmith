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
    // `trim` is still stubbed — verify it reports not-implemented.
    cmd()
        .arg("trim")
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

// -----------------------------------------------------------------------
// Diff --budget tests
// -----------------------------------------------------------------------

#[test]
fn diff_budget_stdout_produces_output() {
    let dir = setup_git_repo();
    cmd()
        .args([
            "diff",
            "--root",
            dir.path().to_str().unwrap(),
            "--budget",
            "5000",
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn diff_budget_creates_manifest_sibling() {
    let dir = setup_git_repo();
    let out_file = dir.path().join("ctx.md");

    cmd()
        .args([
            "diff",
            "--root",
            dir.path().to_str().unwrap(),
            "--budget",
            "5000",
            "--out",
            out_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Manifest should be created as sibling.
    let manifest_path = dir.path().join("ctx.manifest.json");
    assert!(
        manifest_path.exists(),
        "manifest file should exist at {}",
        manifest_path.display()
    );

    // Manifest should be valid JSON with expected fields.
    let content = std::fs::read_to_string(&manifest_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed["summary"]["total_tokens"].is_number());
    assert!(parsed["entries"].is_array());
}

#[test]
fn diff_small_budget_still_includes_one_snippet() {
    let dir = setup_git_repo();
    // Budget of 1 token — should still include at least one snippet.
    cmd()
        .args([
            "diff",
            "--root",
            dir.path().to_str().unwrap(),
            "--budget",
            "1",
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

// -----------------------------------------------------------------------
// Pack command tests
// -----------------------------------------------------------------------

/// Helper: create a JSON bundle file from the test git repo.
fn create_json_bundle(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let bundle_path = dir.path().join("bundle.json");
    cmd()
        .args([
            "diff",
            "--root",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--out",
            bundle_path.to_str().unwrap(),
        ])
        .assert()
        .success();
    bundle_path
}

#[test]
fn pack_reads_json_bundle_to_stdout() {
    let dir = setup_git_repo();
    let bundle_path = create_json_bundle(&dir);

    cmd()
        .args(["pack", bundle_path.to_str().unwrap(), "--stdout"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn pack_with_budget_limits_output() {
    let dir = setup_git_repo();
    let bundle_path = create_json_bundle(&dir);

    // With a large budget, should include everything.
    cmd()
        .args([
            "pack",
            bundle_path.to_str().unwrap(),
            "--budget",
            "50000",
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn pack_with_output_file_creates_manifest() {
    let dir = setup_git_repo();
    let bundle_path = create_json_bundle(&dir);
    let out_path = dir.path().join("packed.md");

    cmd()
        .args([
            "pack",
            bundle_path.to_str().unwrap(),
            "--budget",
            "5000",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(out_path.exists());
    let manifest_path = dir.path().join("packed.manifest.json");
    assert!(
        manifest_path.exists(),
        "manifest should be created alongside output"
    );
}

#[test]
fn pack_missing_bundle_errors() {
    cmd()
        .args(["pack", "/tmp/nonexistent_bundle.json", "--stdout"])
        .assert()
        .failure();
}

// -----------------------------------------------------------------------
// Explain command tests
// -----------------------------------------------------------------------

/// Helper: create a manifest by running diff --out with --budget.
fn create_manifest(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let out_path = dir.path().join("ctx.md");
    cmd()
        .args([
            "diff",
            "--root",
            dir.path().to_str().unwrap(),
            "--budget",
            "5000",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .assert()
        .success();
    dir.path().join("ctx.manifest.json")
}

#[test]
fn explain_reads_manifest() {
    let dir = setup_git_repo();
    let manifest_path = create_manifest(&dir);

    cmd()
        .args(["explain", manifest_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("summary:"))
        .stdout(predicate::str::contains("included"));
}

#[test]
fn explain_top_limits_entries() {
    let dir = setup_git_repo();
    let manifest_path = create_manifest(&dir);

    cmd()
        .args(["explain", manifest_path.to_str().unwrap(), "--top", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello.rs"));
}

#[test]
fn explain_show_weights() {
    let dir = setup_git_repo();
    let manifest_path = create_manifest(&dir);

    cmd()
        .args(["explain", manifest_path.to_str().unwrap(), "--show-weights"])
        .assert()
        .success()
        // Should show actual ranking weights now that diff populates weights_used.
        .stdout(predicate::str::contains("Ranking weights:"))
        .stdout(predicate::str::contains("text:"))
        .stdout(predicate::str::contains("diff:"));
}

#[test]
fn explain_missing_file_errors() {
    cmd()
        .args(["explain", "/tmp/nonexistent_manifest.json"])
        .assert()
        .failure();
}

#[test]
fn explain_directory_with_manifest() {
    let dir = setup_git_repo();
    let _manifest_path = create_manifest(&dir);

    // Write a manifest.json in the dir root for directory-based resolution.
    let manifest_dir = dir.path().join("explaindir");
    std::fs::create_dir_all(&manifest_dir).unwrap();
    std::fs::copy(
        dir.path().join("ctx.manifest.json"),
        manifest_dir.join("manifest.json"),
    )
    .unwrap();

    cmd()
        .args(["explain", manifest_dir.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("summary:"));
}

// -----------------------------------------------------------------------
// Collect command tests
// -----------------------------------------------------------------------

#[test]
fn collect_no_mode_shows_error() {
    let dir = setup_git_repo();
    cmd()
        .args(["collect", "--root", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--files, --grep, or --symbol"));
}

#[test]
fn collect_files_reads_file() {
    let dir = setup_git_repo();
    cmd()
        .args([
            "collect",
            "--files",
            "hello.rs",
            "--root",
            dir.path().to_str().unwrap(),
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello, world!"));
}

#[test]
fn collect_files_json_format() {
    let dir = setup_git_repo();
    let output = cmd()
        .args([
            "collect",
            "--files",
            "hello.rs",
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
    assert!(!parsed["sections"].as_array().unwrap().is_empty());
}

#[test]
fn collect_files_missing_file_errors() {
    let dir = setup_git_repo();
    cmd()
        .args([
            "collect",
            "--files",
            "nonexistent.rs",
            "--root",
            dir.path().to_str().unwrap(),
            "--stdout",
        ])
        .assert()
        .failure();
}

#[test]
fn collect_grep_finds_matches() {
    let dir = setup_git_repo();
    cmd()
        .args([
            "collect",
            "--grep",
            "println",
            "--root",
            dir.path().to_str().unwrap(),
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("println"));
}

#[test]
fn collect_grep_no_matches_shows_message() {
    let dir = setup_git_repo();
    cmd()
        .args([
            "collect",
            "--grep",
            "XYZNONEXISTENT999",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("No matching"));
}

#[test]
fn collect_files_with_budget() {
    let dir = setup_git_repo();
    cmd()
        .args([
            "collect",
            "--files",
            "hello.rs",
            "--root",
            dir.path().to_str().unwrap(),
            "--budget",
            "5000",
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn collect_files_output_creates_manifest() {
    let dir = setup_git_repo();
    let out_path = dir.path().join("collected.md");

    cmd()
        .args([
            "collect",
            "--files",
            "hello.rs",
            "--root",
            dir.path().to_str().unwrap(),
            "--budget",
            "5000",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(out_path.exists());
    let manifest_path = dir.path().join("collected.manifest.json");
    assert!(
        manifest_path.exists(),
        "manifest should be created alongside output"
    );
}

#[test]
fn collect_symbol_finds_definitions() {
    let dir = setup_git_repo();
    cmd()
        .args([
            "collect",
            "--symbol",
            "main",
            "--root",
            dir.path().to_str().unwrap(),
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("main"));
}

#[test]
fn collect_symbol_no_matches_shows_message() {
    let dir = setup_git_repo();
    cmd()
        .args([
            "collect",
            "--symbol",
            "NONEXISTENT_SYMBOL_XYZ",
            "--root",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("No matching"));
}

#[test]
fn collect_grep_with_lang_filter() {
    let dir = setup_git_repo();
    // Add a Python file alongside the Rust file.
    std::fs::write(
        dir.path().join("script.py"),
        "def main():\n    print('hello')\n",
    )
    .unwrap();

    cmd()
        .args([
            "collect",
            "--grep",
            "main",
            "--lang",
            "rust",
            "--root",
            dir.path().to_str().unwrap(),
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("main"));
}

// -----------------------------------------------------------------------
// Stats command tests
// -----------------------------------------------------------------------

#[test]
fn stats_repo_scan_shows_file_count() {
    let dir = setup_git_repo();
    cmd()
        .args(["stats", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("files:"));
}

#[test]
fn stats_repo_scan_with_tokens() {
    let dir = setup_git_repo();
    cmd()
        .args(["stats", "--root", dir.path().to_str().unwrap(), "--tokens"])
        .assert()
        .success()
        .stdout(predicate::str::contains("total tokens:"));
}

#[test]
fn stats_repo_scan_by_lang() {
    let dir = setup_git_repo();
    cmd()
        .args(["stats", "--root", dir.path().to_str().unwrap(), "--by-lang"])
        .assert()
        .success()
        .stdout(predicate::str::contains("By language:"));
}

#[test]
fn stats_bundle_mode_reads_manifest() {
    let dir = setup_git_repo();
    let manifest_path = create_manifest(&dir);

    cmd()
        .args(["stats", manifest_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("total tokens:"));
}

#[test]
fn stats_bundle_mode_top_files() {
    let dir = setup_git_repo();
    let manifest_path = create_manifest(&dir);

    cmd()
        .args(["stats", manifest_path.to_str().unwrap(), "--top-files", "5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Top 5 files"));
}
