use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "thanatos")]
#[command(about = "A declarative atomic Linux distro manager")]
#[command(version = "0.1.0")]
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

    let result = match cli.command {
        Commands::Switch { config } => switch(&config),
        Commands::Diff { config } => diff(&config),
        Commands::Rollback { generation } => rollback(generation),
        Commands::List => list(),
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

fn build_config(config_path: &str) -> Result<thanatos::System, Box<dyn std::error::Error>> {
    let path = PathBuf::from(config_path);

    println!("building config at {}...", path.display());

    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&path)
        .status()?;

    if !build_status.success() {
        return Err("config failed to compile".into());
    }

    let manifest = std::fs::read_to_string(path.join("Cargo.toml"))?;
    let bin_name = manifest
        .lines()
        .find(|l| l.starts_with("name"))
        .and_then(|l| l.split('"').nth(1))
        .ok_or("could not determine binary name from Cargo.toml")?
        .to_string();

    let bin_path = path.join("target").join("release").join(&bin_name);

    println!("evaluating config...");

    let output = Command::new(&bin_path).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("config binary failed: {}", stderr).into());
    }

    let json = String::from_utf8(output.stdout)?;
    let system: thanatos::System = serde_json::from_str(&json)?;

    Ok(system)
}

fn switch(config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let system = build_config(config_path)?;
    println!("applying system...");
    system.build()?;
    println!("done.");
    Ok(())
}

fn diff(_config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("diff not yet implemented");
    Ok(())
}

fn rollback(_generation: Option<u32>) -> Result<(), Box<dyn std::error::Error>> {
    println!("rollback not yet implemented");
    Ok(())
}

fn list() -> Result<(), Box<dyn std::error::Error>> {
    println!("list not yet implemented");
    Ok(())
}
