//! Development utilities — NOT for use on production databases.
//!
//! Usage:
//!   synaptic-cli dev reset             # interactive
//!   synaptic-cli dev reset --force     # skip prompts (CI / scripts)

use clap::Subcommand;

#[derive(Subcommand)]
pub enum DevAction {
    /// Wipe all data rows from every table, keeping the schema and migrations.
    /// Also removes themes/sites/, themes/private/, and uploads/ artefacts so
    /// the next `install` starts from a truly clean slate with no orphan dirs.
    Reset {
        /// Skip the confirmation prompt (useful for scripting).
        #[arg(long)]
        force: bool,

        /// Super-admin password (skips interactive prompt — use only in scripts).
        #[arg(long)]
        password: Option<String>,

        /// Root install directory that contains themes/ and uploads/.
        /// Defaults to the INSTALL_DIR environment variable (set automatically
        /// by `synaptic-cli install`).  If neither is provided the filesystem
        /// cleanup step is skipped and only the database is wiped.
        #[arg(long, env = "INSTALL_DIR")]
        install_dir: Option<String>,

        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL")]
        database_url: Option<String>,
    },
}

pub async fn run(action: DevAction) -> anyhow::Result<()> {
    match action {
        DevAction::Reset { force, password, install_dir, database_url } => {
            reset(force, password, install_dir, database_url).await
        }
    }
}

async fn reset(
    force: bool,
    password: Option<String>,
    install_dir: Option<String>,
    database_url: Option<String>,
) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }

    let pool = super::connect_db().await?;

    // ── Verify super_admin password first ─────────────────────────────────────

    println!();
    println!("  !! DEV RESET — DESTRUCTIVE OPERATION !!");
    println!("  NEVER run this on a production database.");
    println!();

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT password_hash FROM users WHERE is_protected = TRUE AND deleted_at IS NULL LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB error looking up super_admin: {e}"))?;

    let hash = match row {
        Some((h,)) => h,
        None => anyhow::bail!(
            "No super_admin found — the database appears to already be reset.\n\
             Run 'synaptic-cli install' to set up a fresh installation."
        ),
    };

    let supplied = match password {
        Some(p) => p,
        None => dialoguer::Password::new()
            .with_prompt("Super-admin password")
            .interact()
            .map_err(|e| anyhow::anyhow!("Password prompt failed: {e}"))?,
    };

    verify_password(&supplied, &hash)?;

    // ── Gather info to show the user before final confirm ─────────────────────

    let sites: Vec<(String, String)> = sqlx::query_as(
        "SELECT id::text, hostname FROM sites ORDER BY created_at"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    let post_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM posts WHERE post_type = 'post'"
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    let media_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    // ── Print summary ─────────────────────────────────────────────────────────

    println!();
    println!("  ── What will be wiped ──────────────────────────────────");

    if sites.is_empty() {
        println!("  Sites       : (none)");
    } else {
        for (id, hostname) in &sites {
            println!("  Site        : {} ({})", hostname, id);
        }
    }
    println!("  Users       : {}", user_count);
    println!("  Posts       : {}", post_count);
    println!("  Media items : {}", media_count);
    println!();

    // Filesystem paths that will be cleaned.
    if let Some(ref dir) = install_dir {
        let themes_sites = format!("{dir}/themes/sites/");
        let themes_priv  = format!("{dir}/themes/private/");
        let uploads      = format!("{dir}/uploads/");
        println!("  Install dir : {dir}");
        println!("  Filesystem  : {themes_sites}   (all UUID subdirs)");
        println!("                {themes_priv}  (all subdirs)");
        println!("                {uploads}        (all uploaded files)");
    } else {
        println!("  Filesystem  : INSTALL_DIR not set — DB only, no file cleanup.");
        println!("                Pass --install-dir or set INSTALL_DIR in .env to");
        println!("                also remove themes/sites/, themes/private/, uploads/.");
    }

    println!();
    println!("  Database schema and migration history will be preserved.");
    println!("  Documentation table will be preserved (skill-generated, not site data).");
    println!();

    // ── Final confirmation ────────────────────────────────────────────────────

    if !force {
        print!("  Type 'yes' to reset or 'cancel' to abort: ");
        use std::io::Write as _;
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        match input.trim() {
            "yes" => {}
            _ => {
                println!("Aborted.");
                return Ok(());
            }
        }
    }

    // ── Truncate database ─────────────────────────────────────────────────────
    // _sqlx_migrations is intentionally kept so install doesn't re-run migrations.
    // `documentation` is intentionally kept — it holds skill-generated docs that
    // are not site data and should survive dev resets.
    sqlx::query(
        "TRUNCATE TABLE
            tower_sessions,
            post_meta,
            post_taxonomies,
            site_users,
            site_settings,
            form_blocks,
            form_submissions,
            app_settings,
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

    println!("Database cleared.");

    // ── Filesystem cleanup ────────────────────────────────────────────────────

    if let Some(ref dir) = install_dir {
        let base = std::path::Path::new(dir);

        remove_subdirs(&base.join("themes").join("sites"),  "themes/sites/");
        remove_subdirs(&base.join("themes").join("private"), "themes/private/");
        // Uploads stores files flat (not in subdirs), so remove files directly.
        remove_files(&base.join("uploads"), "uploads/");
    }

    println!();
    println!("Reset complete. Next: synaptic-cli install");

    Ok(())
}

/// Delete every immediate child file of `parent`, leaving the parent itself
/// and any subdirectories in place.  Skips directories (e.g. future subfolders).
fn remove_files(parent: &std::path::Path, label: &str) {
    if !parent.is_dir() {
        return;
    }
    let entries = match std::fs::read_dir(parent) {
        Ok(e) => e,
        Err(e) => {
            println!("  Warning: could not read {label}: {e}");
            return;
        }
    };
    let mut removed = 0u32;
    for entry in entries.flatten() {
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            match std::fs::remove_file(entry.path()) {
                Ok(()) => removed += 1,
                Err(e) => println!(
                    "  Warning: could not remove {}: {e}",
                    entry.path().display()
                ),
            }
        }
    }
    if removed > 0 {
        println!("  Removed {removed} file{} from {label}", if removed == 1 { "" } else { "s" });
    } else {
        println!("  {label} already empty — nothing to remove.");
    }
}

/// Delete every immediate child directory of `parent`, leaving the parent
/// itself in place.  Skips non-directory entries (e.g. .gitkeep files).
fn remove_subdirs(parent: &std::path::Path, label: &str) {
    if !parent.is_dir() {
        return;
    }
    let entries = match std::fs::read_dir(parent) {
        Ok(e) => e,
        Err(e) => {
            println!("  Warning: could not read {label}: {e}");
            return;
        }
    };
    let mut removed = 0u32;
    for entry in entries.flatten() {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            match std::fs::remove_dir_all(entry.path()) {
                Ok(()) => removed += 1,
                Err(e) => println!(
                    "  Warning: could not remove {}: {e}",
                    entry.path().display()
                ),
            }
        }
    }
    if removed > 0 {
        println!("  Removed {removed} director{} from {label}", if removed == 1 { "y" } else { "ies" });
    } else {
        println!("  {label} already empty — nothing to remove.");
    }
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
