use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod cage;
mod manifest;
mod vault;
mod registry;

use commands::*;

#[derive(Parser)]
#[command(name = "warden")]
#[command(about = "The official build tool and package manager for Restrict Language", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Restrict Language project
    New {
        /// Project name
        name: String,
    },
    
    /// Initialize a Restrict Language project in current directory
    Init,
    
    /// Add a dependency to the project
    Add {
        /// Dependency specification (name@version, path, or URL)
        dep: String,
        /// Local path to dependency
        #[arg(long)]
        path: Option<String>,
        /// Git repository URL
        #[arg(long)]
        git: Option<String>,
        /// WASM module URL
        #[arg(long)]
        wasm: Option<String>,
        /// WIT interface URL
        #[arg(long)]
        wit: Option<String>,
    },
    
    /// Remove a dependency from the project
    Remove {
        /// Dependency name
        name: String,
    },
    
    /// Build the project
    Build {
        /// Build in release mode with optimizations
        #[arg(long)]
        release: bool,
        /// Watch for changes and rebuild
        #[arg(long)]
        watch: bool,
        /// Build as WASM Component
        #[arg(long)]
        component: bool,
        /// Verify signatures of dependencies
        #[arg(long)]
        verify: bool,
        /// Reproducible build
        #[arg(long)]
        repro: bool,
    },
    
    /// Build and run the project
    Run {
        /// Arguments to pass to the program
        args: Vec<String>,
    },
    
    /// Run tests
    Test {
        /// Test filter
        filter: Option<String>,
    },
    
    /// Publish a package to WardHub
    Publish {
        /// Registry URL
        #[arg(long)]
        registry: Option<String>,
    },
    
    /// Wrap external WASM into a Cage
    Wrap {
        /// Path to WASM module
        wasm: String,
        /// Package name
        #[arg(long)]
        name: String,
        /// Package version
        #[arg(long)]
        version: String,
        /// Path to WIT file
        #[arg(long)]
        wit: Option<String>,
        /// Output path
        #[arg(short, long)]
        output: Option<String>,
    },
    
    /// Unwrap a Cage to extract WASM and WIT
    Unwrap {
        /// Path to Cage file
        cage: String,
        /// Extract as WASM Component
        #[arg(long)]
        component: bool,
        /// Output directory
        #[arg(short, long)]
        output: Option<String>,
    },
    
    /// Check project for issues
    Doctor,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::New { name } => {
            new_project(&name)?;
        }
        Commands::Init => {
            init_project()?;
        }
        Commands::Add { dep, path, git, wasm, wit } => {
            add_dependency(&dep, path, git, wasm, wit).await?;
        }
        Commands::Remove { name } => {
            remove_dependency(&name)?;
        }
        Commands::Build { release, watch, component, verify, repro } => {
            build_project(release, watch, component, verify, repro).await?;
        }
        Commands::Run { args } => {
            run_project(args).await?;
        }
        Commands::Test { filter } => {
            test_project(filter).await?;
        }
        Commands::Publish { registry } => {
            publish_package(registry).await?;
        }
        Commands::Wrap { wasm, name, version, wit, output } => {
            wrap_wasm(&wasm, &name, &version, wit, output)?;
        }
        Commands::Unwrap { cage, component, output } => {
            unwrap_cage(&cage, component, output)?;
        }
        Commands::Doctor => {
            doctor_check().await?;
        }
    }
    
    Ok(())
}