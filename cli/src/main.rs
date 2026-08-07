mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "synap",
    about = "SynapCMS — installer & manager",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive installation wizard (DB init, admin user, Caddyfile, systemd service)
    #[command(after_help = "\
Examples:
  # Interactive install — prompts for everything, including whether to
  # bootstrap a local database or set up Caddy/systemd (both default to no)
  synap install

  # Non-interactive install against an already-working database (e.g. the
  # flow install-vps.sh drives remotely — this is what it calls under the hood)
  DATABASE_URL=postgres://user:pass@localhost/db synap install --non-interactive \\
    --domain example.com --admin-email admin@example.com --admin-username admin \\
    --app-name \"My Site\"

  # Local install: let synap create the Postgres role/database for you
  # (requires sudo access to run commands as the 'postgres' user)
  synap install --bootstrap-db --db-user synaptic --db-name synaptic_signals

  # Local install that also copies the generated Caddyfile/systemd unit into
  # place and starts the service on this machine (requires sudo; any live
  # /etc/caddy/Caddyfile or synapcms.service unit is backed up first)
  synap install --setup-service

  # Fully non-interactive local install: bootstrap the DB, seed the admin/site,
  # and stand up the service, no prompts at all
  synap install --non-interactive --domain example.com --admin-email admin@example.com \\
    --admin-username admin --app-name \"My Site\" --bootstrap-db --setup-service

  # --setup-service with explicit binaries (skips the target/release ->
  # target/debug autodetect — useful if you built somewhere else)
  synap install --setup-service \\
    --synaptic-bin target/release/synaptic --synap-bin target/release/synap
")]
    Install(commands::install::InstallArgs),
    /// Run pending database migrations
    Migrate(commands::migrate::MigrateArgs),
    /// Local dev process management (start/stop/restart/status/logs) — a synap-native
    /// alternative to ./app.sh for the server lifecycle commands. Run from the project root.
    App {
        #[command(subcommand)]
        action: commands::app::AppAction,
    },
    /// Rebuild and reinstall synap itself (cargo install --path cli --force).
    /// Run from the project root after changing CLI source.
    UpdateCli,
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
    /// Security management (per-site IP allow/block lists)
    Security {
        #[command(subcommand)]
        action: commands::security::SecurityAction,
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
        Commands::App { action } => commands::app::run(action).await?,
        Commands::UpdateCli => commands::update_cli::run()?,
        Commands::Dev { action } => commands::dev::run(action).await?,
        Commands::User { action } => commands::user::run(action).await?,
        Commands::Plugin { action } => commands::plugin::run(action).await?,
        Commands::Theme { action } => commands::theme::run(action).await?,
        Commands::Site { action } => commands::site::run(action).await?,
        Commands::Security { action } => commands::security::run(action).await?,
        Commands::Caddy { action } => commands::caddy::run(action)?,
    }

    Ok(())
}
