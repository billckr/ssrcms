//! CLI commands for multi-site management.
//!
//! Usage:
//!   synap-cli site init --hostname <domain>    # backfill existing single-site install
//!   synap-cli site create --hostname <domain>  # add a new empty site
//!   synap-cli site list                        # list all sites
//!   synap-cli site delete --id <uuid>          # remove a site and all its content

use clap::Subcommand;
use uuid::Uuid;

#[derive(Subcommand)]
pub enum SiteAction {
    /// Initialize multi-site support for an existing single-site install.
    /// Creates the first site row and backfills all existing content with its site_id.
    /// Run this once after applying migrations 0008-0011.
    Init {
        /// Hostname for the primary site (e.g. example.com)
        #[arg(long)]
        hostname: String,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Create a new empty site.
    Create {
        /// Hostname for the new site (e.g. client.example.com)
        #[arg(long)]
        hostname: String,
        /// Path to the install directory (e.g. /opt/synaptic-signals) so the
        /// default theme can be seeded into sites/{uuid}/themes/default/ and
        /// the uploads directory can be created at uploads/{uuid}/.
        /// If omitted the directory setup is skipped.
        #[arg(long)]
        install_dir: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// List all sites with their post counts.
    List {
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
    /// Delete a site and all its content (cascade).
    Delete {
        /// UUID of the site to delete
        #[arg(long)]
        id: String,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
}

pub async fn run(action: SiteAction) -> anyhow::Result<()> {
    match action {
        SiteAction::Init { hostname, database_url } => init(hostname, database_url).await,
        SiteAction::Create { hostname, install_dir, database_url } => create(hostname, install_dir, database_url).await,
        SiteAction::List { database_url } => list(database_url).await,
        SiteAction::Delete { id, database_url } => delete(id, database_url).await,
    }
}

async fn init(hostname: String, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        // SAFETY: CLI runs single-threaded during arg parsing; safe to mutate env here.
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;

    // Check that no sites exist yet.
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sites")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    if existing > 0 {
        anyhow::bail!(
            "Sites already exist in the database. Use 'site create' to add additional sites."
        );
    }

    let hostname = hostname.trim().to_lowercase();

    // Create the site row.
    let site_id: Uuid = sqlx::query_scalar(
        "INSERT INTO sites (hostname) VALUES ($1) RETURNING id"
    )
    .bind(&hostname)
    .fetch_one(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create site: {e}"))?;

    println!("Created site '{}' with id {}", hostname, site_id);

    // Backfill content tables.
    let posts_updated: u64 = sqlx::query(
        "UPDATE posts SET site_id = $1 WHERE site_id IS NULL"
    )
    .bind(site_id)
    .execute(&pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    let taxa_updated: u64 = sqlx::query(
        "UPDATE taxonomies SET site_id = $1 WHERE site_id IS NULL"
    )
    .bind(site_id)
    .execute(&pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    let media_updated: u64 = sqlx::query(
        "UPDATE media SET site_id = $1 WHERE site_id IS NULL"
    )
    .bind(site_id)
    .execute(&pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    let settings_updated: u64 = sqlx::query(
        "UPDATE site_settings SET site_id = $1 WHERE site_id IS NULL"
    )
    .bind(site_id)
    .execute(&pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    println!("Backfilled: {} posts, {} taxonomies, {} media, {} settings",
        posts_updated, taxa_updated, media_updated, settings_updated);

    // Add all existing users to the new site with their current role.
    let users: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, role FROM users"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut users_added = 0u64;
    for (user_id, role) in &users {
        let r = sqlx::query(
            "INSERT INTO site_users (site_id, user_id, role) VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING"
        )
        .bind(site_id)
        .bind(user_id)
        .bind(role)
        .execute(&pool)
        .await;
        if r.is_ok() {
            users_added += 1;
        }
    }

    println!("Added {} users to site '{}'", users_added, hostname);

    // Set the protected super_admin as site owner if one exists.
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE is_protected = TRUE AND deleted_at IS NULL LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();

    if let Some(owner_id) = owner {
        sqlx::query(
            "UPDATE sites SET owner_user_id = $1 WHERE id = $2 AND owner_user_id IS NULL"
        )
        .bind(owner_id)
        .bind(site_id)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set site owner: {e}"))?;
        println!("Site owner set to protected super_admin ({}).", owner_id);
        // Set the owner's default_site_id if not already set.
        sqlx::query(
            "UPDATE users SET default_site_id = $1, updated_at = NOW() WHERE id = $2 AND default_site_id IS NULL"
        )
        .bind(site_id)
        .bind(owner_id)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set default site: {e}"))?;    } else {
        println!("No protected super_admin found — owner_user_id left NULL. Backfill with:\n  UPDATE sites SET owner_user_id = '<user-uuid>' WHERE id = '{}';", site_id);
    }

    // Now that all site_settings rows have a non-null site_id, we can upgrade
    // the site_settings primary key from single-column (key) to composite (site_id, key).
    // This allows multiple sites to each have their own copy of every setting key.
    let drop_pk = sqlx::query("ALTER TABLE site_settings DROP CONSTRAINT IF EXISTS site_settings_pkey")
        .execute(&pool)
        .await;
    let add_pk = sqlx::query("ALTER TABLE site_settings ADD PRIMARY KEY (site_id, key)")
        .execute(&pool)
        .await;

    if drop_pk.is_ok() && add_pk.is_ok() {
        println!("Upgraded site_settings primary key to (site_id, key).");
    } else {
        println!("Warning: could not upgrade site_settings PK. This is non-fatal for single-site installs.");
    }

    println!("\nMulti-site initialization complete.");
    println!("Restart Synaptic Signals to apply changes.");

    Ok(())
}

async fn create(hostname: String, install_dir: Option<String>, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        // SAFETY: CLI runs single-threaded during arg parsing; safe to mutate env here.
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;
    let hostname = hostname.trim().to_lowercase();

    let site_id: Uuid = sqlx::query_scalar(
        "INSERT INTO sites (hostname) VALUES ($1) RETURNING id"
    )
    .bind(&hostname)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
            anyhow::anyhow!("A site with hostname '{}' already exists.", hostname)
        } else {
            anyhow::anyhow!("Failed to create site: {e}")
        }
    })?;

    println!("Created site '{}' with id {}", hostname, site_id);

    // Auto-assign the protected super_admin as owner.
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE is_protected = TRUE AND deleted_at IS NULL LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();

    if let Some(owner_id) = owner {
        sqlx::query(
            "UPDATE sites SET owner_user_id = $1 WHERE id = $2 AND owner_user_id IS NULL"
        )
        .bind(owner_id)
        .bind(site_id)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set site owner: {e}"))?;
        println!("Site owner set to protected super_admin ({}).", owner_id);
        // Set the owner's default_site_id if not already set.
        sqlx::query(
            "UPDATE users SET default_site_id = $1, updated_at = NOW() WHERE id = $2 AND default_site_id IS NULL"
        )
        .bind(site_id)
        .bind(owner_id)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set default site: {e}"))?;
    } else {
        println!("No protected super_admin found — owner_user_id left NULL.");
        println!("Backfill with: UPDATE sites SET owner_user_id = '<user-uuid>' WHERE id = '{}'", site_id);
    }

    // Create the site's directories and seed the default theme.
    if let Some(ref base) = install_dir {
        let site_themes_dst = std::path::Path::new(base)
            .join("sites").join(site_id.to_string()).join("themes").join("default");
        let site_uploads_dst = std::path::Path::new(base)
            .join("uploads").join(site_id.to_string());

        if let Err(e) = std::fs::create_dir_all(&site_uploads_dst) {
            println!("Warning: could not create uploads/{}: {}", site_id, e);
        } else {
            println!("Created uploads/{}/", site_id);
        }

        let theme_src = std::path::Path::new(base).join("themes").join("global").join("default");
        if theme_src.is_dir() {
            match copy_dir_all(&theme_src, &site_themes_dst) {
                Ok(()) => println!("Default theme seeded to sites/{}/themes/default/", site_id),
                Err(e) => println!(
                    "Warning: could not copy default theme ({}). \
                     Copy themes/global/default/ to sites/{}/themes/default/ manually.",
                    e, site_id
                ),
            }
        } else {
            println!(
                "Note: themes/global/default/ not found. \
                 Copy it to sites/{}/themes/default/ manually.",
                site_id
            );
        }
    } else {
        println!(
            "Note: pass --install-dir <path> to automatically create site directories \
             and seed the default theme."
        );
    }

    Ok(())
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

async fn list(database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        // SAFETY: CLI runs single-threaded during arg parsing; safe to mutate env here.
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;

    let rows: Vec<(Uuid, String, i64)> = sqlx::query_as(
        r#"SELECT s.id, s.hostname,
              (SELECT COUNT(*) FROM posts p WHERE p.site_id = s.id AND p.post_type = 'post') AS post_count
           FROM sites s
           ORDER BY s.created_at"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to list sites: {e}"))?;

    if rows.is_empty() {
        println!("No sites found. Run 'synap-cli site init --hostname <domain>' to get started.");
        return Ok(());
    }

    println!("\n{:<38} {:<30} {}", "ID", "Hostname", "Posts");
    println!("{}", "-".repeat(74));
    for (id, hostname, posts) in &rows {
        println!("{:<38} {:<30} {}", id, hostname, posts);
    }
    println!();

    Ok(())
}

async fn delete(id_str: String, database_url: Option<String>) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        // SAFETY: CLI runs single-threaded during arg parsing; safe to mutate env here.
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;

    let id: Uuid = id_str.parse()
        .map_err(|_| anyhow::anyhow!("'{}' is not a valid UUID.", id_str))?;

    // Confirm the site exists.
    let hostname: Option<String> = sqlx::query_scalar(
        "SELECT hostname FROM sites WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB error: {e}"))?;

    let hostname = hostname.ok_or_else(|| anyhow::anyhow!("No site with id '{}' found.", id))?;

    // Prompt for confirmation.
    print!("Delete site '{}' ({}) and ALL its content? [y/N] ", hostname, id);
    use std::io::Write as _;
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    if input.trim().to_lowercase() != "y" {
        println!("Aborted.");
        return Ok(());
    }

    sqlx::query("DELETE FROM sites WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to delete site: {e}"))?;

    println!("Site '{}' deleted.", hostname);
    Ok(())
}
