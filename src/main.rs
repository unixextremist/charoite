mod cli;
mod search;

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use clap::Parser;

use crate::cli::Cli;

#[derive(Clone, Copy)] 
enum BuildSystem {
    Make,
    Cargo,
    CMake,
    Unknown,
}

enum OsType {
    LinuxGnu,
    LinuxBusybox,
    BSD,
    BSDLinux,
}

enum InstallPath {
    UsrBin,
    UsrLocalBin,
    LocalBin,
}

fn detect_build_system() -> BuildSystem {
    if Path::new("Cargo.toml").exists() {
        return BuildSystem::Cargo;
    }
    if Path::new("CMakeLists.txt").exists() {
        return BuildSystem::CMake;
    }
    if Path::new("Makefile").exists() {
        return BuildSystem::Make;
    }
    BuildSystem::Unknown
}

fn detect_os() -> OsType {
    if !Path::new("/etc/os-release").exists() {
        println!("No /etc/os-release found. Select OS type:");
        println!("1) GNU/Linux");
        println!("2) BusyBox/Linux");
        println!("3) BSD");
        println!("4) BSD/Linux (e.g., Chimera)");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        return match input.trim() {
            "1" => OsType::LinuxGnu,
            "2" => OsType::LinuxBusybox,
            "3" => OsType::BSD,
            "4" => OsType::BSDLinux,
            _ => {
                println!("Invalid choice, using GNU/Linux");
                OsType::LinuxGnu
            }
        };
    }
    if let Ok(contents) = fs::read_to_string("/etc/os-release") {
        if contents.contains("FreeBSD") {
            return OsType::BSD;
        }
    }
    OsType::LinuxGnu
}

fn get_install_path() -> InstallPath {
    println!("Select installation path:");
    println!("1) /usr/bin (requires sudo)");
    println!("2) /usr/local/bin (requires sudo)");
    println!("3) ~/.local/bin");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    match input.trim() {
        "1" => InstallPath::UsrBin,
        "2" => InstallPath::UsrLocalBin,
        "3" => InstallPath::LocalBin,
        _ => {
            println!("Invalid choice, using ~/.local/bin");
            InstallPath::LocalBin
        }
    }
}

fn run_command(cmd: &str, args: &[&str], elevate: bool) -> io::Result<()> {
    let mut command = if elevate {
        let mut c = Command::new("sudo");
        c.arg(cmd);
        c.args(args);
        c
    } else {
        let mut c = Command::new(cmd);
        c.args(args);
        c
    };
    command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .and_then(|status| if status.success() {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "Command failed"))
        })
}

fn check_tool(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn build_project(build_system: BuildSystem, os_type: &OsType) -> io::Result<()> {
    match build_system {
        BuildSystem::Cargo => {
            if !check_tool("cargo") {
                println!("Cargo not found. Install Rust toolchain? (y/n)");
                io::stdout().flush().unwrap();
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                if input.trim().eq_ignore_ascii_case("y") {
                    run_command("sh", &["-c", "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"], false)?;
                } else {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "Cargo required"));
                }
            }
            run_command("cargo", &["build", "--release"], false)
        }
        BuildSystem::CMake => {
            if !check_tool("cmake") {
                println!("CMake not found. Install CMake? (y/n)");
                io::stdout().flush().unwrap();
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                if input.trim().eq_ignore_ascii_case("y") {
                    match os_type {
                        OsType::BSD | OsType::BSDLinux => run_command("pkg", &["install", "cmake"], true),
                        _ => run_command("sh", &["-c", "if command -v apt >/dev/null; then sudo apt install cmake; elif command -v dnf >/dev/null; then sudo dnf install cmake; elif command -v pacman >/dev/null; then sudo pacman -S cmake; else echo 'Unsupported package manager'; exit 1; fi"], false),
                    }?;
                } else {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "CMake required"));
                }
            }
            run_command("cmake", &["."], false)?;
            run_command("cmake", &["--build", "."], false)
        }
        BuildSystem::Make => {
            let make_cmd = match os_type {
                OsType::BSD | OsType::BSDLinux => {
                    if !check_tool("gmake") {
                        println!("gmake not found. Install gmake? (y/n)");
                io::stdout().flush().unwrap();
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                if input.trim().eq_ignore_ascii_case("y") {
                    run_command("pkg", &["install", "gmake"], true)?;
                } else {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "gmake required"));
                }
            }
            "gmake"
                }
                _ => {
                    if !check_tool("make") {
                        println!("make not found. Install make? (y/n)");
                        io::stdout().flush().unwrap();
                        let mut input = String::new();
                        io::stdin().read_line(&mut input).unwrap();
                        if input.trim().eq_ignore_ascii_case("y") {
                            match os_type {
                                OsType::BSD | OsType::BSDLinux => run_command("pkg", &["install", "gmake"], true),
                                _ => run_command("sh", &["-c", "if command -v apt >/dev/null; then sudo apt install make; elif command -v dnf >/dev/null; then sudo dnf install make; elif command -v pacman >/dev/null; then sudo pacman -S make; else echo 'Unsupported package manager'; exit 1; fi"], false),
                            }?;
                        } else {
                            return Err(io::Error::new(io::ErrorKind::NotFound, "make required"));
                        }
                    }
                    "make"
                }
            };
            run_command(make_cmd, &["all"], false)
        }
        BuildSystem::Unknown => Err(io::Error::new(io::ErrorKind::NotFound, "No build system detected")),
    }
}

fn install_project(build_system: BuildSystem, os_type: &OsType, install_path: InstallPath) -> io::Result<()> {
    let (path, elevate) = match install_path {
        InstallPath::UsrBin => ("/usr/bin".to_string(), true),
        InstallPath::UsrLocalBin => ("/usr/local/bin".to_string(), true),
        InstallPath::LocalBin => {
            let home = env::var("HOME").unwrap();
            (format!("{}/.local/bin", home), false)
        }
    };
    match build_system {
        BuildSystem::Cargo => run_command("cargo", &["install", "--path", ".", "--root", &path], elevate),
        BuildSystem::Make => {
            let make_cmd = match os_type {
                OsType::BSD | OsType::BSDLinux => "gmake",
                _ => "make",
            };
            let prefix_arg = format!("PREFIX={}", path);
            run_command(make_cmd, &["install", &prefix_arg], elevate)
        }
        BuildSystem::CMake => run_command("cmake", &["--install", "."], elevate),
        BuildSystem::Unknown => Err(io::Error::new(io::ErrorKind::NotFound, "No build system detected")),
    }
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        crate::cli::Command::Install { repo } => {
            let url = format!("https://github.com/{}", repo);
            let build_dir = "/tmp/charoite/builds";
            fs::create_dir_all(build_dir)?;
            let repo_name = repo.split('/').last().unwrap();
            let repo_name = repo_name.trim_end_matches(".git");
            let clone_path = format!("{}/{}", build_dir, repo_name);
            if Path::new(&clone_path).exists() {
                fs::remove_dir_all(&clone_path)?;
            }
            run_command("git", &["clone", &url, &clone_path], false)?;
            env::set_current_dir(&clone_path)?;
            let build_system = detect_build_system();
            if let BuildSystem::Unknown = build_system {
                return Err(io::Error::new(io::ErrorKind::NotFound, "Unsupported build system"));
            }
            let os_type = detect_os();
            build_project(build_system, &os_type)?;
            let install_path = get_install_path();
            install_project(build_system, &os_type, install_path)?;
            println!("Installation complete");
            Ok(())
        }
        crate::cli::Command::Search { query } => {
            search::search(&query);
            Ok(())
        }
    }
}
