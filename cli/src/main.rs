mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "synap-cli",
    about = "Synaptic Signals CMS — installer & manager",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive installation wizard (DB init, admin user, Caddyfile, systemd service)
    Install(commands::install::InstallArgs),
    /// Run pending database migrations
    Migrate(commands::migrate::MigrateArgs),
    /// Development utilities (destructive — do not use in production)
    Dev {
        #[command(subcommand)]
        action: commands::dev::DevAction,
    },
    /// User management
    User {
        #[command(subcommand)]
        action: commands::user::UserAction,
    },
    /// Plugin management
    Plugin {
        #[command(subcommand)]
        action: commands::plugin::PluginAction,
    },
    /// Theme management
    Theme {
        #[command(subcommand)]
        action: commands::theme::ThemeAction,
    },
    /// Site management (multi-site support)
    Site {
        #[command(subcommand)]
        action: commands::site::SiteAction,
    },
    /// Caddy permission management (SSL provisioning from admin panel)
    Caddy {
        #[command(subcommand)]
        action: commands::caddy::CaddyAction,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if present (non-fatal if missing)
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    match cli.command {
        Commands::Install(args) => commands::install::run(args).await?,
        Commands::Migrate(args) => commands::migrate::run(args).await?,
        Commands::Dev { action } => commands::dev::run(action).await?,
        Commands::User { action } => commands::user::run(action).await?,
        Commands::Plugin { action } => commands::plugin::run(action).await?,
        Commands::Theme { action } => commands::theme::run(action).await?,
        Commands::Site { action } => commands::site::run(action).await?,
        Commands::Caddy { action } => commands::caddy::run(action)?,
    }

    Ok(())
}
