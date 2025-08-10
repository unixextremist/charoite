use std::io;
use std::path::Path;
use std::process::Command;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct InstalledPackage {
    pub name: String,
    pub source: Option<String>,
    pub build_system: String,
    pub location: String,
    pub build_file: Option<String>,
    pub hash: Option<String>,
    pub version: Option<String>,
    pub last_commit_hash: Option<String>,
    pub install_date: Option<String>,
    pub last_commit_date: Option<String>,
}

pub fn check_deps(deps: &[String]) {
    for dep in deps {
        if !check_dependency(dep) {
            eprintln!("Dependency not found: {}", dep);
            std::process::exit(1);
        }
    }
}

pub fn check_dependency(dep: &str) -> bool {
    if dep == "pkg-config" {
        return check_pkg_config();
    }
    let status = Command::new("which")
        .arg(dep)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !status {
        if check_pkg_config() {
            return Command::new("pkg-config")
                .arg("--exists")
                .arg(dep)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
        }
    }
    status
}

fn check_pkg_config() -> bool {
    Command::new("pkg-config")
        .arg("--version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn get_privilege_command() -> String {
    if Path::new("/usr/bin/doas").exists() {
        "doas".to_string()
    } else {
        "sudo".to_string()
    }
}

pub fn get_git_commit_hash(path: &Path) -> io::Result<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(path)
        .output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "Failed to get commit hash"))
    }
}

pub fn get_git_commit_date(path: &Path) -> io::Result<String> {
    let output = Command::new("git")
        .arg("log")
        .arg("-1")
        .arg("--format=%cd")
        .arg("--date=format:%y-%m-%d")
        .current_dir(path)
        .output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "Failed to get commit date"))
    }
}
