use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use ansi_term::Colour::Green;
use serde_yaml;
use crate::utils::{self, InstalledPackage};

pub fn remove_package(name: &str) -> io::Result<()> {
    let etc_path = Path::new("/etc/charoite");
    let installed_path = etc_path.join("installed.yaml");
    if !installed_path.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "No packages installed"));
    }

    let content = fs::read_to_string(&installed_path)?;
    let mut installed: Vec<InstalledPackage> = serde_yaml::from_str(&content)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    if let Some(pkg) = installed.iter().find(|p| p.name == name) {
        let path = Path::new(&pkg.location);
        if !path.exists() {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("File not found: {}", pkg.location)));
        }

        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        let system_dirs = [
            Path::new("/usr/bin"),
            Path::new("/usr/local/bin"),
            Path::new("/bin"),
            Path::new("/sbin"),
            Path::new("/usr/sbin"),
        ];
        let use_sudo = system_dirs.contains(&parent);

        let status = if use_sudo {
            Command::new(utils::get_privilege_command())
                .arg("rm")
                .arg("-f")
                .arg(&pkg.location)
                .status()
        } else {
            Command::new("rm")
                .arg("-f")
                .arg(&pkg.location)
                .status()
        };

        if let Ok(status) = status {
            if status.success() {
                installed.retain(|p| p.name != name);
                let temp_path = Path::new("/tmp").join("charoite-installed.yaml");
                let content = serde_yaml::to_string(&installed)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                fs::write(&temp_path, content)?;
                Command::new(utils::get_privilege_command())
                    .arg("mv")
                    .arg(&temp_path)
                    .arg(&installed_path)
                    .status()?;
                println!("{}: Removed {}", Green.paint("Success"), name);
                Ok(())
            } else {
                Err(io::Error::new(io::ErrorKind::Other, "Failed to remove file"))
            }
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "Failed to remove file"))
        }
    } else {
        Err(io::Error::new(io::ErrorKind::NotFound, format!("Package {} not found", name)))
    }
}
