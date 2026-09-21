use std::process::{Command, Stdio};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub program: &'static str,
    pub args: Vec<&'static str>,
    pub needs_root: bool,
}

impl Plan {
    pub fn description(&self) -> String {
        format!(
            "{}{} {}",
            if self.needs_root { "sudo " } else { "" },
            self.program,
            self.args.join(" ")
        )
    }
}

pub fn available() -> bool {
    Command::new("oathtool")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn detect() -> Result<Plan, String> {
    let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    plan(std::env::consts::OS, &os_release)
}

fn plan(os: &str, release: &str) -> Result<Plan, String> {
    if os == "macos" {
        return Ok(Plan {
            program: "brew",
            args: vec!["install", "oath-toolkit"],
            needs_root: false,
        });
    }
    if os == "linux" {
        let families: Vec<_> = release
            .lines()
            .filter_map(|line| line.split_once('='))
            .filter(|(key, _)| matches!(*key, "ID" | "ID_LIKE"))
            .flat_map(|(_, value)| value.trim_matches(['\'', '"']).split_whitespace())
            .collect();
        let (program, args) = if families.contains(&"arch") || families.contains(&"omarchy") {
            (
                "pacman",
                vec!["-S", "--needed", "--noconfirm", "oath-toolkit"],
            )
        } else if families.contains(&"debian") || families.contains(&"ubuntu") {
            ("apt-get", vec!["install", "-y", "oathtool"])
        } else if families.contains(&"fedora") || families.contains(&"rhel") {
            ("dnf", vec!["install", "-y", "oathtool"])
        } else {
            return Err("Automatic OTP setup is unavailable on this system. Install OATH Toolkit (oathtool) with your package manager, then retry.".into());
        };
        return Ok(Plan {
            program,
            args,
            needs_root: true,
        });
    }
    Err(
        "Automatic OTP setup supports Linux and macOS. Install oathtool manually on this system."
            .into(),
    )
}

/// Run outside raw/alternate-screen mode so sudo and package-manager prompts work.
/// Only fixed package names and arguments are used; no shell or download scripts.
pub fn install(plan: &Plan) -> Result<(), String> {
    if available() {
        return Ok(());
    }
    let root = Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|out| out.status.success() && out.stdout == b"0\n");
    println!("PassTUI OTP setup: {}", plan.description());
    let mut command = if plan.needs_root && !root {
        let mut command = Command::new("sudo");
        command.arg("--").arg(plan.program);
        command
    } else {
        Command::new(plan.program)
    };
    let status = command.args(&plan.args).status().map_err(|e| {
        format!("Could not start OTP installer: {e}. Install oathtool manually and retry.")
    })?;
    if !status.success() {
        return Err("OTP installation failed or was cancelled. Check network access and package-manager permissions, then retry.".into());
    }
    if !available() {
        return Err("Installation finished, but oathtool is not available on PATH. Restart PassTUI after fixing PATH.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chooses_native_packages_without_shell_commands_or_system_upgrades() {
        for (release, expected, package) in [
            ("ID=omarchy\nID_LIKE=arch", "pacman", "oath-toolkit"),
            ("ID=ubuntu\nID_LIKE=debian", "apt-get", "oathtool"),
            ("ID=mint\nID_LIKE=\"ubuntu debian\"", "apt-get", "oathtool"),
            ("ID=fedora", "dnf", "oathtool"),
        ] {
            let plan = plan("linux", release).unwrap();
            assert_eq!(plan.program, expected);
            assert_eq!(plan.args.last(), Some(&package));
            assert!(plan.needs_root);
        }
        assert_eq!(plan("macos", "").unwrap().program, "brew");
        assert!(!plan("macos", "").unwrap().needs_root);
        assert!(plan("linux", "ID=unknown").is_err());
        assert!(plan("windows", "").is_err());
    }
}
