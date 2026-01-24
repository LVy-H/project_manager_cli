use crate::config::Config;
use anyhow::Result;
use fs_err as fs;
use std::process::Command;

pub fn init_project(config: &Config, name: &str, project_type: &str) -> Result<()> {
    let projects_dir = config.resolve_path("projects");
    if !projects_dir.exists() {
        fs::create_dir_all(&projects_dir)?;
    }

    let project_path = projects_dir.join(name);
    if project_path.exists() {
        anyhow::bail!("Project directory already exists: {:?}", project_path);
    }

    fs::create_dir(&project_path)?;
    println!("Created project directory: {:?}", project_path);

    match project_type {
        "rust" => init_rust(&project_path, name)?,
        "python" => init_python(&project_path, name)?,
        "node" | "js" | "ts" => init_node(&project_path, name)?,
        _ => {
            println!("Unknown type '{}'. Created empty project.", project_type);
            fs::File::create(project_path.join("README.md"))?;
        }
    }

    // Initialize git
    if Command::new("git")
        .arg("init")
        .current_dir(&project_path)
        .status()
        .is_ok()
    {
        println!("Initialized git repository.");
    }

    Ok(())
}

fn init_rust(path: &std::path::Path, name: &str) -> Result<()> {
    // cargo init --name name
    Command::new("cargo")
        .arg("init")
        .arg("--name")
        .arg(name)
        .current_dir(path)
        .status()?;
    Ok(())
}

fn init_python(path: &std::path::Path, _name: &str) -> Result<()> {
    // Basic python structure
    fs::create_dir(path.join("src"))?;
    fs::create_dir(path.join("tests"))?;
    fs::write(path.join("src/__init__.py"), "")?;
    fs::write(
        path.join("main.py"),
        "def main():\n    print('Hello World')\n\nif __name__ == '__main__':\n    main()",
    )?;
    fs::write(path.join("requirements.txt"), "")?;
    fs::write(path.join("README.md"), format!("# {}\n", _name))?;
    // .gitignore
    fs::write(
        path.join(".gitignore"),
        "__pycache__/\n*.pyc\nvenv/\n.env\n",
    )?;

    // Suggest venv
    println!("Suggest running: python -m venv venv");
    Ok(())
}

fn init_node(path: &std::path::Path, _name: &str) -> Result<()> {
    // npm init -y
    Command::new("npm")
        .arg("init")
        .arg("-y")
        .current_dir(path)
        .status()?;

    // Structure
    fs::create_dir(path.join("src"))?;
    fs::write(path.join("src/index.js"), "console.log('Hello World');")?;
    fs::write(path.join(".gitignore"), "node_modules/\n.env\n")?;

    Ok(())
}
