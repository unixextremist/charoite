use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(name = "charoite", version = "0.1.0", author = "")]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Install {
        repo: String,
    },
    Search {
        query: String,
    },
}
