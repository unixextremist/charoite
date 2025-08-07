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
        #[clap(short, long)]
        local: bool,
        #[clap(long)]
        gitlab: bool,
        #[clap(long)]
        codeberg: bool,
        #[clap(short, long)]
        branch: Option<String>,
        #[clap(short, long)]
        patches: Option<String>,
        #[clap(short, long, num_args = 1..)]
        flags: Vec<String>,
        #[clap(short, long)]
        yes: bool,
    },
    Search {
        query: String,
    },
}
