use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

// ── CLI Definition ──

#[derive(Parser)]
#[command(name = "nugget", about = "AI memory layer for Claude Code")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new brain directory
    Init {
        /// Path to create the brain (default: ./brain)
        #[arg(long, default_value = "brain")]
        path: PathBuf,
    },
}

// ── Main ──

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path } => {
            if let Err(e) = nugget_store::brain::init(&path) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
            println!("initialized brain at {}", path.display());
        }
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_init_creates_structure() {
        let tmp = TempDir::new().unwrap();
        let brain_path = tmp.path().join("brain");

        nugget_store::brain::init(&brain_path).unwrap();

        assert!(brain_path.join("brain.yaml").exists());
        assert!(brain_path.join("domains").is_dir());
        assert!(brain_path.join(".gitignore").exists());

        let yaml = fs::read_to_string(brain_path.join("brain.yaml")).unwrap();
        assert_eq!(yaml, "version: 1\n");
    }

    #[test]
    fn test_init_with_explicit_path() {
        let tmp = TempDir::new().unwrap();
        let brain_path = tmp.path().join("custom/location/brain");

        nugget_store::brain::init(&brain_path).unwrap();

        assert!(brain_path.join("brain.yaml").exists());
        assert!(brain_path.join("domains").is_dir());
    }
}
