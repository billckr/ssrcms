//! Development utilities — NOT for use on production databases.
//!
//! Usage:
//!   synaptic-cli dev reset             # interactive confirmation
//!   synaptic-cli dev reset --force     # skip prompt (CI / scripts)

use clap::Subcommand;

#[derive(Subcommand)]
pub enum DevAction {
    /// Wipe all data rows from every table, keeping the schema and migrations.
    /// Run this before `synaptic-cli install` to get a clean slate during dev.
    Reset {
        /// Skip the confirmation prompt (useful for scripting).
        /// Password is still required unless --password is also provided.
        #[arg(long)]
        force: bool,

        /// Super-admin password (skips interactive prompt — use only in scripts).
        #[arg(long)]
        password: Option<String>,

        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL")]
        database_url: Option<String>,
    },
}

pub async fn run(action: DevAction) -> anyhow::Result<()> {
    match action {
        DevAction::Reset { force, password, database_url } => reset(force, password, database_url).await,
    }
}

async fn reset(force: bool, password: Option<String>, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }

    println!();
    println!("  !! DEV RESET — DESTRUCTIVE OPERATION !!");
    println!("  This will DELETE ALL DATA in every table.");
    println!("  The schema and migration history are preserved.");
    println!("  NEVER run this on a production database.");
    println!();

    let pool = super::connect_db().await?;

    // ── Verify super_admin password ───────────────────────────────────────────
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT password_hash FROM users WHERE is_protected = TRUE AND deleted_at IS NULL LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB error looking up super_admin: {e}"))?;

    let hash = match row {
        Some((h,)) => h,
        None => anyhow::bail!("No protected super_admin found in the database."),
    };

    let supplied = match password {
        Some(p) => p,
        None => dialoguer::Password::new()
            .with_prompt("Super-admin password")
            .interact()
            .map_err(|e| anyhow::anyhow!("Password prompt failed: {e}"))?,
    };

    verify_password(&supplied, &hash)?;
    println!("Password verified.");

    // ── Confirmation prompt ───────────────────────────────────────────────────
    if !force {
        print!("  Type 'yes' to continue: ");
        use std::io::Write as _;
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if input.trim() != "yes" {
            println!("Aborted.");
            return Ok(());
        }
    }

    // ── Truncate ───────────────────────────────────────────────────────────────
    // _sqlx_migrations is intentionally kept so install doesn't re-run migrations.
    sqlx::query(
        "TRUNCATE TABLE
            tower_sessions,
            post_meta,
            post_taxonomies,
            site_users,
            site_settings,
            posts,
            media,
            taxonomies,
            sites,
            users
         RESTART IDENTITY CASCADE"
    )
    .execute(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("Truncate failed: {e}"))?;

    println!("All data tables cleared. Schema and migrations intact.");
    println!();
    println!("Next: synaptic-cli install");

    Ok(())
}

fn verify_password(supplied: &str, hash: &str) -> anyhow::Result<()> {
    use argon2::{
        password_hash::{PasswordHash, PasswordVerifier},
        Argon2,
    };
    let parsed = PasswordHash::new(hash)
        .map_err(|e| anyhow::anyhow!("Invalid password hash in DB: {e}"))?;
    Argon2::default()
        .verify_password(supplied.as_bytes(), &parsed)
        .map_err(|_| anyhow::anyhow!("Incorrect password."))
}
