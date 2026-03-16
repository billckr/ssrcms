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
    /// Rename a site's hostname — updates DB records, Caddyfile, embedded post
    /// content URLs, and the hostname symlink in uploads/.
    Rename {
        /// UUID of the site to rename
        #[arg(long)]
        id: String,
        /// New hostname (e.g. newdomain.com)
        #[arg(long)]
        hostname: String,
        /// Path to the Caddyfile to update
        #[arg(long, default_value = "/etc/caddy/Caddyfile")]
        caddyfile: String,
        /// Install directory containing the uploads/ folder (for symlink update).
        /// Defaults to the INSTALL_DIR environment variable set by synap-cli install.
        #[arg(long, env = "INSTALL_DIR")]
        install_dir: Option<String>,
        /// Database URL (overrides DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL", hide = true)]
        database_url: Option<String>,
    },
}

pub async fn run(action: SiteAction) -> anyhow::Result<()> {
    match action {
        SiteAction::Init   { hostname, database_url } => init(hostname, database_url).await,
        SiteAction::Create { hostname, install_dir, database_url } => create(hostname, install_dir, database_url).await,
        SiteAction::List   { database_url } => list(database_url).await,
        SiteAction::Delete { id, database_url } => delete(id, database_url).await,
        SiteAction::Rename { id, hostname, caddyfile, install_dir, database_url } =>
            rename(id, hostname, caddyfile, install_dir, database_url).await,
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

        // Create hostname symlink: uploads/{hostname} → uploads/{uuid}/
        let sym_path = std::path::Path::new(base).join("uploads").join(&hostname);
        if !sym_path.exists() {
            match std::os::unix::fs::symlink(&site_uploads_dst, &sym_path) {
                Ok(()) => println!("Created symlink uploads/{} -> uploads/{}/", hostname, site_id),
                Err(e) => println!("Warning: could not create upload symlink: {}", e),
            }
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

async fn rename(
    id_str: String,
    new_hostname: String,
    caddyfile: String,
    install_dir: Option<String>,
    database_url: Option<String>,
) -> anyhow::Result<()> {
    if let Some(url) = database_url {
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var("DATABASE_URL", url); }
    }
    let pool = super::connect_db().await?;

    let id: Uuid = id_str.parse()
        .map_err(|_| anyhow::anyhow!("'{}' is not a valid UUID.", id_str))?;

    // Fetch current hostname.
    let old_hostname: Option<String> = sqlx::query_scalar(
        "SELECT hostname FROM sites WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB error: {e}"))?;

    let old_hostname = old_hostname
        .ok_or_else(|| anyhow::anyhow!("No site with id '{}' found.", id))?;

    let new_hostname = new_hostname.trim().to_lowercase();
    if old_hostname == new_hostname {
        println!("Site '{}' already uses that hostname — nothing to do.", old_hostname);
        return Ok(());
    }

    // Check the new hostname isn't already taken by another site.
    let conflict: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM sites WHERE hostname = $1"
    )
    .bind(&new_hostname)
    .fetch_optional(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB error: {e}"))?;

    if let Some(other) = conflict {
        if other != id {
            anyhow::bail!("Hostname '{}' is already used by another site ({}).", new_hostname, other);
        }
    }

    println!();
    println!("  Rename site:");
    println!("    ID:           {}", id);
    println!("    Old hostname: {}", old_hostname);
    println!("    New hostname: {}", new_hostname);
    println!();
    println!("  This will update:");
    println!("    • sites.hostname");
    println!("    • site_settings site_url");
    println!("    • posts.content — /uploads/{} → /uploads/{}", old_hostname, new_hostname);
    println!("    • Caddyfile block header: {}", caddyfile);
    if let Some(ref dir) = install_dir {
        println!("    • uploads/ symlink in: {}/uploads/", dir);
    }
    println!();
    println!("  Note: hostname text manually typed into post body content (not via");
    println!("  the media picker) will NOT be automatically updated.");
    println!();

    print!("  Proceed? [y/N] ");
    use std::io::Write as _;
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    if input.trim().to_lowercase() != "y" {
        println!("Aborted.");
        return Ok(());
    }

    // 1. Update sites.hostname.
    sqlx::query("UPDATE sites SET hostname = $1, updated_at = NOW() WHERE id = $2")
        .bind(&new_hostname)
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update hostname: {e}"))?;
    println!("  ✓ Updated sites.hostname");

    // 2. Update site_settings site_url.
    let new_url = format!("http://{}", new_hostname);
    sqlx::query(
        "UPDATE site_settings SET value = $1 WHERE site_id = $2 AND key = 'site_url'"
    )
    .bind(&new_url)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update site_url: {e}"))?;
    println!("  ✓ Updated site_settings site_url to {}", new_url);

    // 3. Update embedded media upload URLs in posts.content.
    let old_prefix = format!("/uploads/{}/", old_hostname);
    let new_prefix = format!("/uploads/{}/", new_hostname);
    let updated_posts = sqlx::query(
        "UPDATE posts SET content = REPLACE(content, $1, $2) \
         WHERE site_id = $3 AND content LIKE '%' || $1 || '%'"
    )
    .bind(&old_prefix)
    .bind(&new_prefix)
    .bind(id)
    .execute(&pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);
    if updated_posts > 0 {
        println!("  ✓ Updated {} post(s) with embedded upload URLs", updated_posts);
    } else {
        println!("  ✓ No embedded upload URLs to update in posts");
    }

    // 4. Update Caddyfile.
    match update_caddyfile(&caddyfile, &old_hostname, &new_hostname) {
        Ok(true) => {
            println!("  ✓ Updated Caddyfile: {} → {}", old_hostname, new_hostname);
            // Reload Caddy.
            let reload = std::process::Command::new("sudo")
                .args(["caddy", "reload", "--config", &caddyfile, "--adapter", "caddyfile"])
                .status();
            match reload {
                Ok(s) if s.success() => println!("  ✓ Caddy reloaded"),
                Ok(s) => println!("  Warning: caddy reload exited with {}", s),
                Err(e) => println!("  Warning: could not reload Caddy: {}", e),
            }
        }
        Ok(false) => println!(
            "  ✓ Caddyfile: no block for '{}' found (may not be configured yet)",
            old_hostname
        ),
        Err(e) => {
            println!("  Warning: could not update Caddyfile: {}", e);
            println!("    Manually replace '{}' with '{}' in {}", old_hostname, new_hostname, caddyfile);
        }
    }

    // 5. Update hostname symlink in uploads/.
    if let Some(ref dir) = install_dir {
        let uploads = std::path::Path::new(dir).join("uploads");
        let old_sym = uploads.join(&old_hostname);
        let new_sym = uploads.join(&new_hostname);
        let target  = uploads.join(id.to_string());

        if old_sym.is_symlink() {
            if let Err(e) = std::fs::remove_file(&old_sym) {
                println!("  Warning: could not remove old symlink: {}", e);
            }
        }
        if target.is_dir() && !new_sym.exists() {
            match std::os::unix::fs::symlink(&target, &new_sym) {
                Ok(()) => println!("  ✓ Symlink: uploads/{} -> uploads/{}/", new_hostname, id),
                Err(e) => {
                    println!("  Warning: could not create new symlink: {}", e);
                    println!("    Manually: ln -s {}/{} {}/{}", uploads.display(), id, uploads.display(), new_hostname);
                }
            }
        }
    } else {
        println!("  ℹ  --install-dir not provided — skipping symlink update.");
        println!("     Pass --install-dir <path> or set INSTALL_DIR to update the symlink.");
    }

    println!();
    println!("Rename complete. Restart Synaptic Signals to apply the new hostname.");
    println!();
    println!("Note: hostname text manually typed into post body content was not");
    println!("automatically updated. Review posts for any references to '{}'.", old_hostname);

    Ok(())
}

fn update_caddyfile(path: &str, old: &str, new_host: &str) -> std::io::Result<bool> {
    let content = std::fs::read_to_string(path)?;
    // Replace the site block header and log file path.
    let updated = content
        .replace(&format!("{} {{", old), &format!("{} {{", new_host))
        .replace(&format!("{}.log", old), &format!("{}.log", new_host));
    if updated == content {
        return Ok(false);
    }
    std::fs::write(path, &updated)?;
    Ok(true)
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
