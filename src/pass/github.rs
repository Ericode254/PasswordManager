use std::path::Path;
use std::process::Command;

fn command_error(command: &str, error: std::io::Error) -> String {
    format!("Failed to run `{command}`: {error}")
}

fn output_error(command: &str, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let message = if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    if message.is_empty() {
        format!("`{command}` failed")
    } else {
        format!("`{command}` failed: {message}")
    }
}

pub fn is_available() -> bool {
    Command::new("gh")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

pub fn is_authenticated() -> Result<bool, String> {
    let output = Command::new("gh")
        .args(["auth", "status", "--hostname", "github.com"])
        .output()
        .map_err(|error| command_error("gh auth status", error))?;
    Ok(output.status.success())
}

pub fn login() -> Result<String, String> {
    let command = "gh auth login --web --git-protocol ssh";
    let output = Command::new("gh")
        .args([
            "auth",
            "login",
            "--hostname",
            "github.com",
            "--git-protocol",
            "ssh",
            "--web",
        ])
        .output()
        .map_err(|error| command_error(command, error))?;

    if !output.status.success() {
        return Err(output_error(command, &output));
    }

    Ok("GitHub login completed".to_string())
}

pub fn create_private_repo(store_dir: &Path, name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Repository name cannot be empty".to_string());
    }

    let store_path = store_dir
        .to_str()
        .ok_or_else(|| "Password store path is not valid UTF-8".to_string())?;
    let command = "gh repo create --private --source --remote origin --push";
    let output = Command::new("gh")
        .args([
            "repo",
            "create",
            name,
            "--private",
            "--source",
            store_path,
            "--remote",
            "origin",
            "--push",
        ])
        .output()
        .map_err(|error| command_error(command, error))?;

    if !output.status.success() {
        return Err(output_error(command, &output));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = if !stdout.trim().is_empty() {
        stdout.trim()
    } else if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        "Private GitHub repository created and pushed"
    };
    Ok(message.to_string())
}
