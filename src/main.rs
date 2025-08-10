mod cli;
mod install;
mod search;
mod utils;
mod remove;

use std::io;
use std::path::Path;
use clap::Parser;
use crate::cli::{Cli, Command};

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Install { repo, local, gitlab, codeberg, branch, patches, flags, yes } => {
            let patches_path = patches.as_deref().map(Path::new);
            install::install(&repo, local, gitlab, codeberg, branch.as_deref(), patches_path, &flags, yes)
        }
        Command::Search { query } => {
            println!("\x1b[1;35mSearching for {}...\x1b[0m", query);
            search::search(&query);
            Ok(())
        }
        Command::Remove { name } => {
            remove::remove_package(&name)
        }
    }
}
