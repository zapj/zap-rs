use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Add {
        #[arg(short, long)]
        name: String,
    },
    Remove {
        #[arg(short, long)]
        id: u32,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add { name } => println!("Adding {}", name),
        Commands::Remove { id } => println!("Removing {}", id),
    }
}