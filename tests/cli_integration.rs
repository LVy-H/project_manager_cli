use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

struct TestEnv {
    temp_dir: TempDir,
}

impl TestEnv {
    fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        Self { temp_dir }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
        cmd.current_dir(self.temp_dir.path());
        cmd.env("WX_PATHS_WORKSPACE", self.temp_dir.path());
        let config_file = self.temp_dir.path().join("config.yaml");
        if config_file.exists() {
            cmd.arg("--config").arg(&config_file);
        }
        cmd
    }

    fn path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }

    fn setup_workspace(&self) {
        let base = self.path();
        fs::create_dir_all(base.join("0_Inbox")).unwrap();
        fs::create_dir_all(base.join("1_Projects")).unwrap();
        fs::create_dir_all(base.join("2_Areas")).unwrap();
        fs::create_dir_all(base.join("3_Resources")).unwrap();
        fs::create_dir_all(base.join("4_Archives")).unwrap();
        fs::create_dir_all(base.join("1_Projects/CTFs")).unwrap();
    }

    fn create_config(&self) {
        let config_content = format!(
            r#"paths:
  workspace: {}
  inbox: {}/0_Inbox
  projects: {}/1_Projects
  areas: {}/2_Areas
  resources: {}/3_Resources
  archives: {}/4_Archives

organize:
  ctf_dir: 1_Projects/CTFs

ctf:
  default_categories:
    - web
    - pwn
    - crypto
    - rev"#,
            self.path().display(),
            self.path().display(),
            self.path().display(),
            self.path().display(),
            self.path().display(),
            self.path().display()
        );

        fs::write(self.path().join("config.yaml"), config_content).unwrap();
    }
}

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Ward & index your workspace"));
}

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("wardex"));
}

#[test]
fn test_ctf_list_empty() {
    let env = TestEnv::new();
    env.setup_workspace();
    env.create_config();

    env.cmd().args(&["ctf", "list"]).assert().success();
}

#[test]
fn test_config_init() {
    let env = TestEnv::new();
    env.create_config();

    env.cmd().args(&["config", "init"]).assert().success();
}

#[test]
fn test_config_init_twice_without_force_fails() {
    let env = TestEnv::new();
    env.create_config();

    env.cmd().args(&["config", "init"]).assert().success();

    env.cmd()
        .args(&["config", "init"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_config_init_with_force_succeeds() {
    let env = TestEnv::new();
    env.create_config();

    env.cmd().args(&["config", "init"]).assert().success();

    env.cmd()
        .args(&["config", "init", "--force"])
        .assert()
        .success();
}

#[test]
fn test_config_goto_workspace() {
    let env = TestEnv::new();
    env.create_config();

    env.cmd()
        .args(&["config", "goto", "workspace"])
        .assert()
        .success()
        .stdout(predicate::str::contains(env.path().to_str().unwrap()));
}

#[test]
fn test_config_goto_invalid_folder() {
    let env = TestEnv::new();
    env.create_config();

    env.cmd()
        .args(&["config", "goto", "invalid"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown folder"));
}

#[test]
fn test_stats_command() {
    let env = TestEnv::new();
    env.setup_workspace();
    env.create_config();

    fs::write(env.path().join("0_Inbox/test.txt"), "test").unwrap();

    env.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("Workspace"));
}

#[test]
fn test_ctf_init_creates_event() {
    let env = TestEnv::new();
    env.setup_workspace();
    env.create_config();

    std::env::set_current_dir(env.path().join("1_Projects/CTFs")).unwrap();

    env.cmd()
        .args(&["ctf", "init", "TestEvent"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Initialized"));

    let ctf_dirs: Vec<_> = fs::read_dir(env.path().join("1_Projects/CTFs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    assert!(!ctf_dirs.is_empty());
}

#[test]
fn test_ctf_init_with_date() {
    let env = TestEnv::new();
    env.setup_workspace();
    env.create_config();

    std::env::set_current_dir(env.path().join("1_Projects/CTFs")).unwrap();

    env.cmd()
        .args(&["ctf", "init", "TestEvent", "--date", "2024-12-25"])
        .assert()
        .success();

    let event_dir = fs::read_dir(env.path().join("1_Projects/CTFs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().contains("2024"));

    assert!(event_dir.is_some());
}

#[test]
fn test_ctf_add_invalid_format() {
    let env = TestEnv::new();
    env.setup_workspace();
    env.create_config();

    env.cmd()
        .args(&["ctf", "add", "invalid-format"])
        .assert()
        .failure();
}
