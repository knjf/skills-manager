mod commands;

use clap::{Parser, Subcommand};

/// sm — Skills Manager CLI
///
/// Manage AI agent skill scenarios from the terminal.
/// Switches scenarios by syncing skills to agent directories.
#[derive(Parser)]
#[command(name = "sm", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all scenarios with skill counts
    #[command(alias = "ls")]
    List,

    /// Show the active scenario
    #[command(alias = "c")]
    Current,

    /// Switch to a scenario
    #[command(alias = "sw")]
    Switch {
        /// Scenario name to switch to
        name: String,
    },

    /// List skills in a scenario (default: active)
    #[command(alias = "sk")]
    Skills {
        /// Scenario name (defaults to active scenario)
        name: Option<String>,
    },

    /// Compare two scenarios
    #[command(alias = "d")]
    Diff {
        /// First scenario name
        a: String,
        /// Second scenario name
        b: String,
    },

    /// List packs in a scenario (default: active)
    Packs {
        /// Scenario name (defaults to active scenario)
        name: Option<String>,
    },

    /// Manage packs in a scenario
    Pack {
        #[command(subcommand)]
        action: PackAction,
    },
}

#[derive(Subcommand)]
enum PackAction {
    /// Add a pack to a scenario
    Add {
        /// Pack name
        pack: String,
        /// Scenario name
        scenario: String,
    },
    /// Remove a pack from a scenario
    Remove {
        /// Pack name
        pack: String,
        /// Scenario name
        scenario: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::List => commands::cmd_list(),
        Commands::Current => commands::cmd_current(),
        Commands::Switch { name } => commands::cmd_switch(&name),
        Commands::Skills { name } => commands::cmd_skills(name.as_deref()),
        Commands::Diff { a, b } => commands::cmd_diff(&a, &b),
        Commands::Packs { name } => commands::cmd_packs(name.as_deref()),
        Commands::Pack { action } => match action {
            PackAction::Add { pack, scenario } => commands::cmd_pack_add(&pack, &scenario),
            PackAction::Remove { pack, scenario } => commands::cmd_pack_remove(&pack, &scenario),
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
