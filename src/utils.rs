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
}

pub fn check_deps(deps: &[String]) {
    for dep in deps {
        if !check_dep(dep) {
            eprintln!("Dependency not found: {}", dep);
            std::process::exit(1);
        }
    }
}

fn check_dep(dep: &str) -> bool {
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
