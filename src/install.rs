use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;
use ansi_term::Colour::{Green, Red, Yellow};
use serde_json;
use serde_yaml;
use sha2::{Sha256, Digest};
use chrono::Local;
use crate::utils::{self, InstalledPackage, check_dependency};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BuildSystem {
    Make,
    Autotools,
    Cargo,
    Cmake,
    Meson,
    Ninja,
    Nimble,
    Stack,
    Pip,
    Unknown,
}

struct InstallLocation {
    bin_path: PathBuf,
    elevate: bool,
}

pub fn install(
    repo: &str,
    local: bool,
    gitlab: bool,
    codeberg: bool,
    branch: Option<&str>,
    patches: Option<&Path>,
    flags: &[String],
    yes: bool,
) -> io::Result<()> {
    let start = Instant::now();
    let tmp = Path::new("/tmp/charoite");
    let builds = tmp.join("builds");
    let etc = Path::new("/etc/charoite");

    for dir in [tmp, &builds] {
        if !dir.exists() {
            fs::create_dir_all(dir).expect("Failed to create temp directory");
        }
    }

    let source = if codeberg {
        Some("codeberg")
    } else if gitlab {
        Some("gitlab")
    } else {
        None
    };
    let (_, domain) = match source {
        Some("gitlab") => ("gitlab", "gitlab.com"),
        Some("codeberg") => ("codeberg", "codeberg.org"),
        _ => ("github", "github.com")
    };

    let repo_name = repo.split('/').last().unwrap();
    let build_dir = builds.join(repo_name);

    if build_dir.exists() {
        if let Err(e) = fs::remove_dir_all(&build_dir) {
            if e.kind() == io::ErrorKind::PermissionDenied {
                let status = Command::new(utils::get_privilege_command())
                    .arg("rm")
                    .arg("-rf")
                    .arg(&build_dir)
                    .status();
                if status.is_err() || !status.unwrap().success() {
                    eprintln!("{}: Failed to clean previous build", Red.paint("Error"));
                    return Ok(());
                }
            } else {
                eprintln!("{}: Failed to clean previous build: {}", Red.paint("Error"), e);
                return Ok(());
            }
        }
    }

    println!("\x1b[1m~> Cloning repository: {}\x1b[0m", repo);
    let mut git_clone = Command::new("git");
    git_clone
        .arg("clone")
        .arg("--depth=1")
        .arg(format!("https://{}/{}", domain, repo))
        .arg(&build_dir);

    if let Some(b) = branch {
        git_clone.arg("--branch").arg(b);
    }

    let status = git_clone
        .stdout(Stdio::null())
        .status()
        .expect("Git command failed");

    if !status.success() {
        eprintln!("{}", Red.paint("Failed to clone repository"));
        return Ok(());
    }

    if let Some(patches_dir) = patches {
        apply_patches(&build_dir, patches_dir);
    }

    env::set_current_dir(&build_dir)?;
    let (build_system, build_file, deps, custom_flags) = detect_build_system();

    if build_system == BuildSystem::Unknown {
        eprintln!("{}", Red.paint("Unsupported build system"));
        return Ok(());
    }

    println!("~> Build system: {}", match build_system {
        BuildSystem::Make => Green.paint("Make"),
        BuildSystem::Autotools => Green.paint("Autotools"),
        BuildSystem::Cargo => Green.paint("Cargo"),
        BuildSystem::Cmake => Green.paint("CMake"),
        BuildSystem::Meson => Green.paint("Meson"),
        BuildSystem::Ninja => Green.paint("Ninja"),
        BuildSystem::Nimble => Green.paint("Nimble"),
        BuildSystem::Stack => Green.paint("Stack"),
        BuildSystem::Pip => Green.paint("Pip"),
        _ => unreachable!()
    });

    let uses_pkg_config = check_pkg_config_usage(build_system, build_file.as_ref());
    if !uses_pkg_config {
        println!("{}", Yellow.paint("Warning: This project doesn't use pkg-config for dependencies"));
        if !yes {
            print!("~> Proceed anyway? [y/N] ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("{}", Yellow.paint("Build cancelled by user"));
                return Ok(());
            }
        }
    }

    utils::check_deps(&deps);

    let mut final_flags = custom_flags;
    final_flags.extend(flags.iter().map(|s| s.to_string()));

    println!("~> Building with flags: {:?}", final_flags);
    build_project(build_system, &build_dir, &final_flags, build_file.as_ref())?;

    if build_system == BuildSystem::Pip {
        let requirements_file = build_dir.join("requirements.txt");
        if requirements_file.exists() {
            println!("~> Installing Python dependencies");
            let pip_command = if local {
                vec!["pip", "install", "--user", "-r", requirements_file.to_str().unwrap()]
            } else {
                vec!["pip", "install", "-r", requirements_file.to_str().unwrap()]
            };
            let status = if local {
                Command::new(pip_command[0])
                    .args(&pip_command[1..])
                    .status()
            } else {
                Command::new(utils::get_privilege_command())
                    .args(&pip_command)
                    .status()
            };
            if let Ok(status) = status {
                if !status.success() {
                    eprintln!("{}", Red.paint("Failed to install Python dependencies"));
                    return Err(io::Error::new(io::ErrorKind::Other, "Failed to install Python dependencies"));
                }
            } else {
                eprintln!("{}", Red.paint("Failed to run pip"));
                return Err(io::Error::new(io::ErrorKind::Other, "Failed to run pip"));
            }
        }
    }

    println!("~> Installing...");
    let install_location = get_install_path(local);
    install_project(build_system, &install_location, &build_dir, repo_name)?;

    if !local {
        let mut hasher = Sha256::new();
        if let Some(bf) = &build_file {
            if let Ok(content) = fs::read(&build_dir.join(bf)) {
                hasher.update(&content);
            }
        }
        let hash = format!("{:x}", hasher.finalize());
        let mut version = None;
        if build_system == BuildSystem::Cargo {
            if let Ok(cargo_toml) = fs::read_to_string(build_dir.join("Cargo.toml")) {
                if let Some(v) = cargo_toml.lines().find(|l| l.starts_with("version = ")) {
                    version = v.split('"').nth(1).map(|s| s.to_string());
                }
            }
        }
        
        let commit_hash = utils::get_git_commit_hash(&build_dir).ok();
        let commit_date = utils::get_git_commit_date(&build_dir).ok();
        
        let installed_binary_path = install_location.bin_path.join(repo_name);
        
        update_installed_packages(
            repo_name,
            source,
            build_system,
            &installed_binary_path,
            build_file.as_ref(),
            Some(hash),
            version,
            commit_hash,
            Some(Local::now().format("%y-%m-%d").to_string()),
            commit_date,
        );
    }

    println!("{} in {}s", 
        Green.paint("~> INSTALL FINISHED"), 
        start.elapsed().as_secs()
    );

    if !local {
        println!("{}", Yellow.paint("Warning: charoite installs packages to /usr/local/bin by default.\nIf /usr/local/bin is not in your $PATH, you may need to add it."));
    } else {
        println!("{}", Green.paint("Installed to ~/.local/bin. Make sure this directory is in your PATH."));
    }
    Ok(())
}

fn detect_build_system() -> (BuildSystem, Option<String>, Vec<String>, Vec<String>) {
    let mut build_files = Vec::new();
    if Path::new("radon.json").exists() {
        build_files.push(("radon.json", BuildSystem::Unknown));
    }
    if Path::new("charoite.json").exists() {
        build_files.push(("charoite.json", BuildSystem::Unknown));
    }
    if Path::new("Cargo.toml").exists() {
        build_files.push(("Cargo.toml", BuildSystem::Cargo));
    }
    if Path::new("Makefile").exists() {
        build_files.push(("Makefile", BuildSystem::Make));
    }
    if Path::new("configure").exists() {
        build_files.push(("configure", BuildSystem::Autotools));
    }
    if Path::new("CMakeLists.txt").exists() {
        build_files.push(("CMakeLists.txt", BuildSystem::Cmake));
    }
    if Path::new("meson.build").exists() {
        build_files.push(("meson.build", BuildSystem::Meson));
    }
    if Path::new("build.ninja").exists() {
        build_files.push(("build.ninja", BuildSystem::Ninja));
    }
    if fs::read_dir(".")
        .unwrap()
        .any(|e| e.unwrap().path().extension().map(|e| e == "nimble").unwrap_or(false)) 
    {
        build_files.push(("*.nimble", BuildSystem::Nimble));
    }
    if Path::new("stack.yaml").exists() {
        build_files.push(("stack.yaml", BuildSystem::Stack));
    }
    if Path::new("requirements.txt").exists() {
        build_files.push(("requirements.txt", BuildSystem::Pip));
    }
    let (build_file, build_system) = if !build_files.is_empty() {
        if build_files.len() > 1 {
            println!("\x1b[1;36mMultiple build files detected. Select one:\x1b[0m");
            for (i, (file, _)) in build_files.iter().enumerate() {
                println!("{}: {}", i + 1, file);
            }
            io::stdout().flush().unwrap();
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let choice: usize = input.trim().parse().unwrap_or(0);
            if choice > 0 && choice <= build_files.len() {
                build_files[choice - 1]
            } else {
                return (BuildSystem::Unknown, None, vec![], vec![]);
            }
        } else {
            build_files[0]
        }
    } else {
        return (BuildSystem::Unknown, None, vec![], vec![]);
    };
    let (deps, flags) = match build_system {
        BuildSystem::Make => (parse_make_deps(Path::new(".")), vec![]),
        BuildSystem::Autotools => (parse_autotools_deps(Path::new(".")), vec![]),
        BuildSystem::Cargo => (parse_cargo_deps(Path::new(".")), vec![]),
        BuildSystem::Cmake => (vec!["cmake".to_string()], vec![]),
        BuildSystem::Meson => (vec!["meson".to_string(), "ninja".to_string()], vec![]),
        BuildSystem::Ninja => (vec!["ninja".to_string()], vec![]),
        BuildSystem::Nimble => (vec!["nim".to_string(), "nimble".to_string()], vec![]),
        BuildSystem::Stack => (vec!["stack".to_string()], vec![]),
        BuildSystem::Pip => (vec!["pip".to_string()], vec![]),
        _ => (vec![], vec![]),
    };
    if build_file == "radon.json" || build_file == "charoite.json" {
        let (bs, d, f) = parse_charoite_json(Path::new(build_file));
        return (bs, Some(build_file.to_string()), d, f);
    }
    (build_system, Some(build_file.to_string()), deps, flags)
}

fn parse_charoite_json(path: &Path) -> (BuildSystem, Vec<String>, Vec<String>) {
    let file = std::fs::File::open(path).expect("Failed to open charoite.json");
    let reader = std::io::BufReader::new(file);
    let json: serde_json::Value = serde_json::from_reader(reader).expect("Invalid charoite.json");
    let build_system = match json["build_system"].as_str().unwrap_or("make") {
        "make" => BuildSystem::Make,
        "autotools" => BuildSystem::Autotools,
        "cargo" => BuildSystem::Cargo,
        "cmake" => BuildSystem::Cmake,
        "meson" => BuildSystem::Meson,
        "ninja" => BuildSystem::Ninja,
        "nimble" => BuildSystem::Nimble,
        "stack" => BuildSystem::Stack,
        "pip" => BuildSystem::Pip,
        _ => BuildSystem::Unknown,
    };
    let deps = json["dependencies"].as_array().map(|arr| {
        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
    }).unwrap_or_default();
    let flags = json["flags"].as_array().map(|arr| {
        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
    }).unwrap_or_default();
    (build_system, deps, flags)
}

fn parse_make_deps(dir: &Path) -> Vec<String> {
    let makefiles = ["Makefile", "makefile", "GNUMakefile"];
    let found_file = makefiles.iter().find(|f| dir.join(f).exists()).unwrap_or(&"Makefile");
    let makefile = fs::read_to_string(dir.join(found_file)).unwrap_or_default();
    makefile.lines().find(|l| l.contains("# DEPENDENCIES:")).map(|l| {
        l.split(':').nth(1).unwrap().split(',').map(|s| s.trim().to_string()).collect()
    }).unwrap_or_default()
}

fn parse_autotools_deps(dir: &Path) -> Vec<String> {
    let configure = fs::read_to_string(dir.join("configure")).unwrap_or_default();
    let mut deps = Vec::new();
    if configure.contains("PKG_CHECK_MODULES") {
        deps.push("pkg-config".to_string());
    } else {
        println!("{}", Yellow.paint("Warning: Autotools project doesn't use pkg-config"));
    }
    if configure.contains("AC_PROG_CC") {
        deps.push("gcc".to_string());
    }
    if configure.contains("AC_PROG_CXX") {
        deps.push("g++".to_string());
    }
    deps
}

fn parse_cargo_deps(dir: &Path) -> Vec<String> {
    let cargo_toml = fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default();
    let value = cargo_toml.parse::<serde_json::Value>().unwrap_or(serde_json::Value::Null);
    value.get("package").and_then(|p| p.get("metadata")).and_then(|m| m.get("charoite")).and_then(|r| r.get("dependencies")).and_then(|d| d.as_array()).map(|deps| {
        deps.iter().filter_map(|d| d.as_str().map(|s| s.to_string())).collect()
    }).unwrap_or_default()
}

fn check_pkg_config_usage(build_system: BuildSystem, build_file: Option<&String>) -> bool {
    match build_system {
        BuildSystem::Autotools => true,
        BuildSystem::Make => {
            if let Some(file) = build_file {
                let makefile = fs::read_to_string(file).unwrap_or_default();
                if makefile.contains("pkg-config") {
                    return true;
                }
            }
            false
        }
        BuildSystem::Cmake => {
            if let Some(file) = build_file {
                let cmake = fs::read_to_string(file).unwrap_or_default();
                if cmake.contains("pkg_check_modules") ||
                   cmake.contains("pkg_search_module") ||
                   cmake.contains("find_package(PkgConfig") {
                    return true;
                }
            }
            false
        }
        BuildSystem::Meson => {
            if let Some(file) = build_file {
                let meson = fs::read_to_string(file).unwrap_or_default();
                if meson.contains("dependency(") {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

fn get_install_path(local: bool) -> InstallLocation {
    if local {
        let home = env::var("HOME").unwrap();
        let local_bin = PathBuf::from(home).join(".local/bin");
        if !local_bin.exists() {
            fs::create_dir_all(&local_bin).expect("Failed to create local bin directory");
        }
        InstallLocation { bin_path: local_bin, elevate: false }
    } else {
        InstallLocation { bin_path: PathBuf::from("/usr/local/bin"), elevate: true }
    }
}

fn run_command(cmd: &str, args: &[&str], elevate: bool, current_dir: Option<&Path>) -> io::Result<()> {
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
    if let Some(dir) = current_dir {
        command.current_dir(dir);
    }
    command.stdout(Stdio::inherit()).stderr(Stdio::inherit()).status().and_then(|status| {
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "Command failed"))
        }
    })
}

fn apply_patches(build_dir: &Path, patches_dir: &Path) {
    let patches: Vec<PathBuf> = fs::read_dir(patches_dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map(|e| e == "patch").unwrap_or(false)).collect();
    for patch in patches {
        println!("Applying patch: {}", patch.display());
        let status = Command::new("patch")
            .arg("-Np1")
            .arg("--directory")
            .arg(build_dir)
            .arg("--input")
            .arg(&patch)
            .status()
            .expect("Failed to apply patch");
        if !status.success() {
            eprintln!("{}: Failed to apply {}", Red.paint("Error"), patch.display());
        }
    }
}

fn build_project(
    build_system: BuildSystem,
    build_dir: &Path,
    flags: &[String],
    build_file: Option<&String>,
) -> io::Result<()> {
    let final_flags: Vec<&str> = flags.iter().map(|s| s.as_str()).collect();
    match build_system {
        BuildSystem::Make => {
            let makefile = if build_dir.join("BSDMakefile").exists() { "BSDMakefile" } else { "Makefile" };
            run_command("make", &["-f", makefile, &final_flags.join(" ")], false, Some(build_dir))
        }
        BuildSystem::Autotools => {
            run_command("./configure", &final_flags, false, Some(build_dir))?;
            run_command("make", &[], false, Some(build_dir))
        }
        BuildSystem::Cargo => {
            let mut args = vec!["build", "--release"];
            args.extend(final_flags.iter());
            run_command("cargo", &args, false, Some(build_dir))
        }
        BuildSystem::Cmake => {
            let build_path = build_dir.join("build");
            fs::create_dir_all(&build_path)?;
            run_command("cmake", &["-DCMAKE_BUILD_TYPE=Release", ".."], false, Some(&build_path))?;
            run_command("cmake", &["--build", "."], false, Some(&build_path))
        }
        BuildSystem::Meson => {
            let build_path = build_dir.join("build");
            fs::create_dir_all(&build_path)?;
            run_command("meson", &["setup", "build"], false, Some(build_dir))?;
            run_command("ninja", &["-C", "build"], false, Some(build_dir))
        }
        BuildSystem::Ninja => run_command("ninja", &final_flags, false, Some(build_dir)),
        BuildSystem::Nimble => run_command("nimble", &["build", &final_flags.join(" ")], false, Some(build_dir)),
        BuildSystem::Stack => run_command("stack", &["install", &final_flags.join(" "), "--local-bin-path", "bin"], false, Some(build_dir)),
        BuildSystem::Pip => Ok(()),
        _ => Err(io::Error::new(io::ErrorKind::Unsupported, "Unsupported build system")),
    }
}

fn find_executable_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                if let Some(exec) = find_executable_in_dir(&path, name) {
                    return Some(exec);
                }
            } else if path.is_file() {
                if let Some(filename) = path.file_name() {
                    if filename == name {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn install_all_cargo_binaries(install_location: &InstallLocation, build_dir: &Path) -> io::Result<()> {
    let release_dir = build_dir.join("target/release");
    let mut binaries = Vec::new();
    for entry in fs::read_dir(&release_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            binaries.push(path);
        }
    }
    if binaries.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "No binaries found in target/release"));
    }
    for binary_path in binaries {
        let bin_name = binary_path.file_name().unwrap();
        let dest_path = install_location.bin_path.join(bin_name);
        if install_location.elevate {
            run_command("cp", &[binary_path.to_str().unwrap(), dest_path.to_str().unwrap()], true, None)?;
        } else {
            fs::copy(&binary_path, &dest_path)?;
        }
    }
    Ok(())
}

fn install_project(
    build_system: BuildSystem,
    install_location: &InstallLocation,
    build_dir: &Path,
    repo_name: &str,
) -> io::Result<()> {
    match build_system {
        BuildSystem::Cargo => install_all_cargo_binaries(install_location, build_dir),
        BuildSystem::Make => {
            let prefix = install_location.bin_path.parent().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid bin path"))?.to_str().unwrap();
            let prefix_arg = format!("PREFIX={}", prefix);
            run_command("make", &["install", &prefix_arg], install_location.elevate, Some(build_dir))
        }
        BuildSystem::Autotools => run_command("make", &["install"], install_location.elevate, Some(build_dir)),
        BuildSystem::Cmake => run_command("cmake", &["--install", "."], install_location.elevate, Some(&build_dir.join("build"))),
        BuildSystem::Meson | BuildSystem::Ninja => run_command("ninja", &["install"], install_location.elevate, Some(&build_dir.join("build"))),
        BuildSystem::Nimble => run_command("nimble", &["install"], install_location.elevate, Some(build_dir)),
        BuildSystem::Stack => {
            let bin_dir = build_dir.join("bin");
            if let Some(binary) = find_executable_in_dir(&bin_dir, repo_name) {
                let dest_path = install_location.bin_path.join(repo_name);
                if install_location.elevate {
                    run_command("cp", &[binary.to_str().unwrap(), dest_path.to_str().unwrap()], true, None)
                } else {
                    fs::copy(&binary, &dest_path).map(|_| ())
                }
            } else {
                Err(io::Error::new(io::ErrorKind::NotFound, "Binary not found"))
            }
        }
        BuildSystem::Pip => {
            if !check_dependency("pip") {
                return Err(io::Error::new(io::ErrorKind::NotFound, "pip not found"));
            }
            let pip_command = if install_location.elevate {
                vec!["pip", "install", "."]
            } else {
                vec!["pip", "install", "--user", "."]
            };
            let status = if install_location.elevate {
                Command::new(utils::get_privilege_command())
                    .args(&pip_command)
                    .status()
            } else {
                Command::new(pip_command[0])
                    .args(&pip_command[1..])
                    .status()
            };
            if let Ok(status) = status {
                if status.success() {
                    Ok(())
                } else {
                    Err(io::Error::new(io::ErrorKind::Other, "pip install failed"))
                }
            } else {
                Err(io::Error::new(io::ErrorKind::Other, "Failed to run pip"))
            }
        }
        _ => Err(io::Error::new(io::ErrorKind::Unsupported, "Unsupported build system")),
    }
}

fn update_installed_packages(
    repo_name: &str,
    source: Option<&str>,
    build_system: BuildSystem,
    location: &Path,
    build_file: Option<&String>,
    hash: Option<String>,
    version: Option<String>,
    commit_hash: Option<String>,
    install_date: Option<String>,
    last_commit_date: Option<String>,
) {
    let etc_path = Path::new("/etc/charoite");
    if !etc_path.exists() {
        fs::create_dir_all(etc_path).expect("Failed to create /etc/charoite");
    }
    let installed_path = etc_path.join("installed.yaml");
    let mut installed: Vec<InstalledPackage> = if installed_path.exists() {
        let content = fs::read_to_string(&installed_path).unwrap_or_default();
        serde_yaml::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };
    
    let pkg = InstalledPackage {
        name: repo_name.to_string(),
        source: source.map(|s| s.to_string()),
        build_system: format!("{:?}", build_system),
        location: location.to_string_lossy().to_string(),
        build_file: build_file.cloned(),
        hash,
        version,
        last_commit_hash: commit_hash,
        install_date,
        last_commit_date,
    };
    
    installed.retain(|p| p.name != repo_name);
    installed.push(pkg);
    
    let temp_path = Path::new("/tmp").join("charoite-installed.yaml");
    fs::write(&temp_path, serde_yaml::to_string(&installed).unwrap()).unwrap();
    Command::new(&utils::get_privilege_command())
        .arg("mv")
        .arg(&temp_path)
        .arg(&installed_path)
        .status()
        .expect("Failed to update package list");
}
