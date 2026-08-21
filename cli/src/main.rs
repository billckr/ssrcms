mod commands;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

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

  # Local install that also merges this domain's block into the generated
  # Caddyfile/systemd unit and starts the service on this machine (requires
  # sudo; other domains' Caddy blocks are left untouched, the systemd unit
  # file is backed up first)
  synap install --setup-service

  # Fully non-interactive local install: bootstrap the DB, seed the admin/site,
  # and stand up the service, no prompts at all
  synap install --non-interactive --domain example.com --admin-email admin@example.com \\
    --admin-username admin --app-name \"My Site\" --bootstrap-db --setup-service

  # --setup-service with explicit binaries (skips the target/release ->
  # target/debug autodetect — useful if you built somewhere else)
  synap install --setup-service \\
    --synapcms-bin target/release/synapcms --synap-bin target/release/synap

  # If preflight detects an existing install (running process, active
  # service, or existing DB data) you're asked Fresh/Coexist/Bail
  # interactively; non-interactively, declare it up front:
  synap install --non-interactive --on-conflict=coexist \\
    --domain second.example.com --admin-email admin@example.com --admin-username admin

  # Take over completely (stop what's running, wipe existing data) with
  # no prompts — requires an admin password to authorize the wipe
  synap install --non-interactive --on-conflict=fresh --admin-password 'Str0ng!Pw' \\
    --domain example.com --admin-email admin@example.com --admin-username admin
")]
    Install(commands::install::InstallArgs),
    /// Run pending database migrations
    Migrate(commands::migrate::MigrateArgs),
    /// Local dev process management (start/stop/restart/status/logs)
    App {
        #[command(subcommand)]
        action: commands::app::AppAction,
    },
    /// Rebuild and reinstall synap itself
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
    /// Manage sites
    Site {
        #[command(subcommand)]
        action: commands::site::SiteAction,
    },
    /// Search index management
    Search {
        #[command(subcommand)]
        action: commands::search::SearchAction,
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

    let mut command = Cli::command();
    command.build();
    let matches = command
        .mut_subcommand("help", |cmd| cmd.about("Get help with commands subcommands"))
        .get_matches();
    let cli = Cli::from_arg_matches(&matches)?;

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
        Commands::Search { action } => commands::search::run(action).await?,
        Commands::Security { action } => commands::security::run(action).await?,
        Commands::Caddy { action } => commands::caddy::run(action)?,
    }

    Ok(())
}
