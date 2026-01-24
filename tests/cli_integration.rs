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
        cmd.env("XDG_CONFIG_HOME", self.temp_dir.path());
        cmd.env("XDG_DATA_HOME", self.temp_dir.path());
        cmd.env("HOME", self.temp_dir.path()); // Just in case
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
    // Do NOT create config first
    // env.create_config();

    env.cmd().args(&["config", "init"]).assert().success();
}

#[test]
fn test_config_init_twice_without_force_fails() {
    let env = TestEnv::new();
    // Create config manually first
    env.create_config();

    // This should fail because it exists
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

    // First attempt fails
    env.cmd().args(&["config", "init"]).assert().failure();

    // Force succeeds
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

#[test]
fn test_ctf_path_command() {
    let env = TestEnv::new();
    env.setup_workspace();
    env.create_config();

    // Init event
    let ctf_root = env.path().join("1_Projects/CTFs");
    std::fs::create_dir_all(&ctf_root).unwrap();

    // We need to run init inside the ctf root or ensure config points to it
    // Config points to it.

    env.cmd()
        .args(&["ctf", "init", "PathTest"])
        .assert()
        .success();

    // Get path
    env.cmd()
        .args(&["ctf", "path", "PathTest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PathTest"));

    // Find the event dir name to use for add command context
    let event_dir = fs::read_dir(&ctf_root)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().contains("PathTest"))
        .unwrap()
        .path();

    // Add challenge needs to be run inside event dir
    let mut cmd = env.cmd();
    cmd.current_dir(&event_dir);
    cmd.args(&["ctf", "add", "web/chall1"]).assert().success();

    // Test path to challenge
    env.cmd()
        .args(&["ctf", "path", "PathTest", "chall1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("chall1"));
}

#[test]
fn test_ctf_import_with_category_flag_and_move() {
    let env = TestEnv::new();
    env.setup_workspace();
    env.create_config();

    let ctf_root = env.path().join("1_Projects/CTFs");
    std::fs::create_dir_all(&ctf_root).unwrap();

    env.cmd()
        .args(&["ctf", "init", "ImportTest"])
        .assert()
        .success();

    let event_dir = fs::read_dir(&ctf_root)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().contains("ImportTest"))
        .unwrap()
        .path();

    // Create dummy file to import outside event dir (e.g. in Inbox)
    let inbox = env.path().join("0_Inbox");
    let import_file = inbox.join("flag.txt");
    fs::write(&import_file, "CTF{test}").unwrap();

    // Import with category override
    let mut cmd = env.cmd();
    cmd.current_dir(&event_dir);
    cmd.args(&[
        "ctf",
        "import",
        import_file.to_str().unwrap(),
        "--category",
        "misc",
    ])
    .assert()
    .success();

    // Check if file moved and exists in misc/flag
    let challenge_dir = event_dir.join("misc/flag");
    assert!(challenge_dir.exists());
    assert!(challenge_dir.join("flag.txt").exists());

    // Verify file content
    let content = fs::read_to_string(challenge_dir.join("flag.txt")).unwrap();
    assert_eq!(content, "CTF{test}");

    // Check that original file is GONE (moved)
    assert!(!import_file.exists());
}

#[test]
fn test_ctf_context_awareness() {
    let env = TestEnv::new();
    env.setup_workspace();
    env.create_config();

    let ctf_root = env.path().join("1_Projects/CTFs");
    std::fs::create_dir_all(&ctf_root).unwrap();

    // Init event
    env.cmd()
        .args(&["ctf", "init", "ContextTest"])
        .assert()
        .success();

    let event_dir = fs::read_dir(&ctf_root)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().contains("ContextTest"))
        .unwrap()
        .path();

    // 1. Test add from category dir (implicit category)
    let web_dir = event_dir.join("web");
    assert!(web_dir.exists());

    let mut cmd = env.cmd();
    cmd.current_dir(&web_dir);
    cmd.args(&["ctf", "add", "chall1"]) // Should infer "web"
        .assert()
        .success();

    assert!(web_dir.join("chall1").exists());

    // 2. Test info command from deep inside
    let mut cmd = env.cmd();
    cmd.current_dir(&web_dir.join("chall1"));
    cmd.args(&["ctf", "info"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ContextTest"))
        .stdout(predicate::str::contains("web"));
}

#[test]
fn test_ctf_global_state() {
    let env = TestEnv::new();
    env.setup_workspace();
    env.create_config();

    // 1. Init event (should auto-set global state)
    let output = env
        .cmd()
        .args(&["ctf", "init", "GlobalEvent"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("Switched to event") {
        println!("DEBUG: Init stdout: {}", stdout);
        println!(
            "DEBUG: Init stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // 2. Check info from OUTSIDE the event dir (e.g. root workspace)
    let mut cmd = env.cmd();
    cmd.current_dir(env.path()); // Workspace root, not event dir
    cmd.args(&["ctf", "info"])
        .assert()
        .success()
        .stdout(predicate::str::contains("GlobalEvent"))
        .stdout(predicate::str::contains("Global State"));

    // 3. Add challenge from outside (using global state)
    let mut cmd = env.cmd();
    cmd.current_dir(env.path());
    cmd.args(&["ctf", "add", "pwn/remote-exploit"])
        .assert()
        .success();

    // Verify it was created inside the event
    let ctf_root = env.path().join("1_Projects/CTFs");
    // Find the event dir
    let event_dir = fs::read_dir(&ctf_root)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().contains("GlobalEvent"))
        .unwrap()
        .path();

    assert!(event_dir.join("pwn/remote-exploit").exists());

    // 4. Create another event and switch to it
    env.cmd()
        .args(&["ctf", "init", "SecondEvent"])
        .assert()
        .success();
    // Verify switch
    env.cmd()
        .args(&["ctf", "info"])
        .assert()
        .success()
        .stdout(predicate::str::contains("SecondEvent"));

    // 5. Explicitly use the first event
    env.cmd()
        .args(&["ctf", "use", "GlobalEvent"])
        .assert()
        .success();

    env.cmd()
        .args(&["ctf", "info"])
        .assert()
        .success()
        .stdout(predicate::str::contains("GlobalEvent"));
}
