use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Switch {
        #[arg(short, long, default_value = ".")]
        config: String,
    },
    Diff {
        #[arg(short, long, default_value = ".")]
        config: String,
    },
    Rollback {
        #[arg(short, long)]
        generation: Option<u32>,
    },
    List,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Switch { config } => {
            println!("switching to config at {}", config);
        }
        Commands::Diff { config } => {
            println!("diffing config at {}", config);
        }
        Commands::Rollback { generation } => match generation {
            Some(g) => println!("rolling back to generation {}", g),
            None => println!("rolling back to previous generation"),
        },
        Commands::List => {
            println!("listing generations");
        }
    }
}
