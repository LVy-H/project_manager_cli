use anyhow::{Context, Result};
use std::fs;
use std::process::Command;

pub fn init_devcontainer() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let devcontainer_dir = current_dir.join(".devcontainer");

    if devcontainer_dir.exists() {
        println!("Dictionary .devcontainer already exists.");
        return Ok(());
    }

    fs::create_dir(&devcontainer_dir)?;

    let config_content = r#"{
    "name": "Wardex Dev Container",
    "image": "mcr.microsoft.com/devcontainers/rust:1",
    "features": {
        "ghcr.io/devcontainers/features/rust:1": {}
    },
    // "postCreateCommand": "cargo build",
    "customizations": {
        "vscode": {
            "extensions": [
                "rust-lang.rust-analyzer",
                "tamasfe.even-better-toml",
                "serayuzgur.crates"
            ]
        }
    }
}"#;

    let config_path = devcontainer_dir.join("devcontainer.json");
    fs::write(&config_path, config_content)?;

    println!("Created .devcontainer/devcontainer.json");
    Ok(())
}

pub fn list_images() -> Result<()> {
    // Basic docker images list for now.
    // In future could filter by project name or use docker scout

    println!("🐳 Docker Images (local)");
    println!("{}", "-".repeat(60));

    let output = Command::new("docker")
        .arg("images")
        .arg("--format")
        .arg("table {{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedSince}}")
        .output()
        .context("Failed to execute docker command. Is docker installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Docker error: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Skip header line if we want to custom format, but table format is nice.
    // Let's just print it.
    print!("{}", stdout);

    Ok(())
}
