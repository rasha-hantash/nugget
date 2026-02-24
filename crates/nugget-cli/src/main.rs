use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use nugget_inbox::Inbox;
use nugget_store::BrainStore;

// ── Types ──

#[derive(Parser)]
#[command(name = "nugget", about = "Personal knowledge brain")]
struct Cli {
    /// Path to the brain directory (defaults to ./brain)
    #[arg(long, global = true, default_value = "brain")]
    brain: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new brain directory
    Init,

    /// Manage domains
    Domain {
        #[command(subcommand)]
        command: DomainCommands,
    },

    /// List pending inbox items
    Inbox,

    /// Accept inbox items by index (1-based)
    Accept {
        /// Indices of items to accept
        indices: Vec<usize>,
    },

    /// Reject inbox items by index (1-based)
    Reject {
        /// Indices of items to reject
        indices: Vec<usize>,
    },

    /// Interactive one-by-one inbox review
    Review,

    /// Start MCP server (stdio transport for Claude Code)
    Mcp,
}

#[derive(Subcommand)]
enum DomainCommands {
    /// Add a new domain
    Add {
        /// Domain name (e.g., "coding/rust")
        name: String,

        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// List all domains
    List,
}

// ── Helpers ──

fn resolve_brain(path: &Path) -> Result<BrainStore> {
    let path = expand_tilde(path);
    let canonical = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(BrainStore::new(canonical))
}

fn expand_tilde(path: &Path) -> PathBuf {
    if let Ok(stripped) = path.strip_prefix("~") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    path.to_path_buf()
}

fn print_inbox_entry(index: usize, entry: &nugget_inbox::InboxEntry) {
    let item = &entry.item;
    let first_line = item.body.lines().next().unwrap_or("(empty)");
    let truncated = if first_line.len() > 80 {
        format!("{}...", &first_line[..77])
    } else {
        first_line.to_string()
    };

    println!(
        "  [{}] {} | {} | {} | {}",
        index, item.knowledge_type, item.capture_method, item.suggested_domain, truncated
    );
}

// ── Public API ──

fn main() -> Result<()> {
    let cli = Cli::parse();
    let store = resolve_brain(&cli.brain)?;

    match cli.command {
        Commands::Init => cmd_init(&store),
        Commands::Domain { command } => match command {
            DomainCommands::Add { name, description } => {
                cmd_domain_add(&store, &name, description.as_deref())
            }
            DomainCommands::List => cmd_domain_list(&store),
        },
        Commands::Inbox => cmd_inbox(&store),
        Commands::Accept { indices } => cmd_accept(&store, &indices),
        Commands::Reject { indices } => cmd_reject(&store, &indices),
        Commands::Review => cmd_review(&store),
        Commands::Mcp => cmd_mcp(&store),
    }
}

fn cmd_init(store: &BrainStore) -> Result<()> {
    store.init()?;
    println!("Initialized brain at {}", store.root.display());
    Ok(())
}

fn cmd_domain_add(store: &BrainStore, name: &str, description: Option<&str>) -> Result<()> {
    if !store.is_initialized() {
        bail!("brain not initialized — run `nugget init` first");
    }
    let path = store.add_domain(name, description)?;
    println!("Created domain '{}' at {}", name, path.display());
    Ok(())
}

fn cmd_domain_list(store: &BrainStore) -> Result<()> {
    if !store.is_initialized() {
        bail!("brain not initialized — run `nugget init` first");
    }
    let domains = store.list_domains()?;
    if domains.is_empty() {
        println!("No domains yet. Add one with: nugget domain add <name>");
    } else {
        for d in &domains {
            println!("  {}", d);
        }
    }
    Ok(())
}

fn cmd_inbox(store: &BrainStore) -> Result<()> {
    if !store.is_initialized() {
        bail!("brain not initialized — run `nugget init` first");
    }
    let inbox = Inbox::new(store.clone());
    let entries = inbox.list()?;

    if entries.is_empty() {
        println!("Inbox is empty.");
    } else {
        println!("Inbox ({} items):", entries.len());
        for (i, entry) in entries.iter().enumerate() {
            print_inbox_entry(i + 1, entry);
        }
    }
    Ok(())
}

fn cmd_accept(store: &BrainStore, indices: &[usize]) -> Result<()> {
    if !store.is_initialized() {
        bail!("brain not initialized — run `nugget init` first");
    }
    let inbox = Inbox::new(store.clone());
    let accepted = inbox.accept_by_indices(indices)?;
    for path in &accepted {
        println!("Accepted -> {}", path.display());
    }
    Ok(())
}

fn cmd_reject(store: &BrainStore, indices: &[usize]) -> Result<()> {
    if !store.is_initialized() {
        bail!("brain not initialized — run `nugget init` first");
    }
    let inbox = Inbox::new(store.clone());
    inbox.reject_by_indices(indices)?;
    println!("Rejected {} item(s).", indices.len());
    Ok(())
}

fn cmd_mcp(store: &BrainStore) -> Result<()> {
    if !store.is_initialized() {
        eprintln!("nugget: initializing brain at {}", store.root.display());
        store.init()?;
    }
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(nugget_mcp::run_mcp_server(store.clone()))
}

fn cmd_review(store: &BrainStore) -> Result<()> {
    if !store.is_initialized() {
        bail!("brain not initialized — run `nugget init` first");
    }
    let inbox = Inbox::new(store.clone());
    let entries = inbox.list()?;

    if entries.is_empty() {
        println!("Inbox is empty — nothing to review.");
        return Ok(());
    }

    println!("Reviewing {} inbox item(s)...\n", entries.len());

    for (i, entry) in entries.iter().enumerate() {
        println!("--- Item {}/{} ---", i + 1, entries.len());
        println!("Type:    {}", entry.item.knowledge_type);
        println!("Method:  {}", entry.item.capture_method);
        println!("Domain:  {}", entry.item.suggested_domain);
        if !entry.item.tags.is_empty() {
            let tags: Vec<&str> = entry.item.tags.iter().map(|t| t.0.as_str()).collect();
            println!("Tags:    {}", tags.join(", "));
        }
        println!();
        println!("{}", entry.item.body);
        println!();

        print!("[a]ccept / [r]eject / [s]kip / [q]uit? ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim().to_lowercase().as_str() {
            "a" | "accept" => {
                let dest = inbox.accept(entry)?;
                println!("  -> Accepted: {}\n", dest.display());
            }
            "r" | "reject" => {
                inbox.reject(entry)?;
                println!("  -> Rejected\n");
            }
            "q" | "quit" => {
                println!("Stopping review.");
                break;
            }
            _ => {
                println!("  -> Skipped\n");
            }
        }
    }

    Ok(())
}
