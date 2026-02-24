use clap::Args;
use dialoguer::{Confirm, Input, Password};
use uuid::Uuid;

#[derive(Args)]
pub struct InstallArgs {
    /// Skip interactive prompts and use defaults/env vars
    #[arg(long)]
    pub non_interactive: bool,

    /// Output directory for Caddyfile and .service (defaults to current dir)
    #[arg(long, default_value = ".")]
    pub output_dir: String,
}

pub async fn run(args: InstallArgs) -> anyhow::Result<()> {
    println!("\nWelcome to Synaptic Signals CMS Installer");
    println!("==========================================\n");

    // ── Gather configuration ───────────────────────────────────────────────

    let domain: String = Input::new()
        .with_prompt("Domain name (e.g. example.com)")
        .interact_text()?;

    let port: u16 = Input::new()
        .with_prompt("Port Axum listens on")
        .default(3000u16)
        .interact_text()?;

    let install_dir: String = Input::new()
        .with_prompt("Install directory (full path)")
        .default(
            std::env::current_dir()
                .ok()
                .and_then(|p| p.to_str().map(String::from))
                .unwrap_or_else(|| "/opt/synaptic-signals".to_string()),
        )
        .interact_text()?;

    let database_url: String = Input::new()
        .with_prompt("Database URL")
        .default(
            std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://synaptic:password@localhost:5432/synaptic_signals".to_string()),
        )
        .interact_text()?;

    println!("\n── Database ─────────────────────────────────────────────");
    println!("Connecting to database...");

    std::env::set_var("DATABASE_URL", &database_url);
    let pool = super::connect_db().await?;

    println!("Running migrations...");
    sqlx::migrate!("../migrations")
        .run(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Migration failed: {e}"))?;
    println!("Migrations applied.\n");

    // ── Admin user ─────────────────────────────────────────────────────────

    let create_admin = Confirm::new()
        .with_prompt("Create an admin user now?")
        .default(true)
        .interact()?;

    // Track the created admin's UUID so we can link them to the site as owner.
    let mut admin_id: Option<Uuid> = None;

    if create_admin {
        println!("\n── Admin User ───────────────────────────────────────────");

        let username: String = Input::new()
            .with_prompt("Admin username")
            .default("admin".to_string())
            .interact_text()?;

        let email: String = Input::new()
            .with_prompt("Admin email")
            .interact_text()?;

        let display_name: String = Input::new()
            .with_prompt("Display name")
            .default(username.clone())
            .interact_text()?;

        let password = Password::new()
            .with_prompt("Admin password")
            .with_confirmation("Confirm password", "Passwords do not match")
            .interact()?;

        let hash = hash_password(&password)?;
        let id = Uuid::new_v4();
        admin_id = Some(id);

        sqlx::query(
            "INSERT INTO users (id, username, email, display_name, password_hash, role, is_protected, created_at)
             VALUES ($1, $2, $3, $4, $5, 'super_admin', TRUE, NOW())
             ON CONFLICT (email) DO NOTHING"
        )
        .bind(id)
        .bind(&username)
        .bind(&email)
        .bind(&display_name)
        .bind(&hash)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create admin user: {e}"))?;

        println!("Admin user '{}' ({}) created.", username, email);
    }

    // ── Initial site ───────────────────────────────────────────────────────
    // Insert the domain as the first site so the super admin has a default
    // site context on first login. Link the admin as owner if one was created.
    let site_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO sites (id, hostname, owner_user_id, created_at, updated_at)
         VALUES ($1, $2, $3, NOW(), NOW())
         ON CONFLICT (hostname) DO NOTHING"
    )
    .bind(site_id)
    .bind(&domain)
    .bind(admin_id)
    .execute(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create initial site: {e}"))?;
    println!("Initial site '{}' created.", domain);

    // Seed default site_settings so the admin panel shows real values on first login.
    let site_url = format!("http://{domain}");
    let settings_defaults: &[(&str, &str)] = &[
        ("site_name",        &domain),
        ("site_description", ""),
        ("site_url",         &site_url),
        ("site_language",    "en-US"),
        ("active_theme",     "default"),
        ("posts_per_page",   "10"),
        ("date_format",      "%B %-d, %Y"),
    ];
    for (key, value) in settings_defaults {
        sqlx::query(
            "INSERT INTO site_settings (site_id, key, value)
             VALUES ($1, $2, $3)
             ON CONFLICT (site_id, key) WHERE site_id IS NOT NULL DO NOTHING"
        )
        .bind(site_id)
        .bind(key)
        .bind(value)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to seed site_settings: {e}"))?;
    }
    println!("Default site settings seeded.");

    // Link the admin user to their site in site_users so the switcher works.
    if let Some(uid) = admin_id {
        sqlx::query(
            "INSERT INTO site_users (site_id, user_id, role)
             VALUES ($1, $2, 'super_admin')
             ON CONFLICT DO NOTHING"
        )
        .bind(site_id)
        .bind(uid)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to link admin to site: {e}"))?;
        println!("Admin linked to site '{}' as owner.", domain);
    }

    // ── Deployment files ───────────────────────────────────────────────────

    let uploads_dir = format!("{}/uploads", install_dir);
    let theme_dir = format!("{}/themes", install_dir);
    let output_dir = std::path::Path::new(&args.output_dir);

    println!("\n── Deployment Files ─────────────────────────────────────");

    write_caddyfile(output_dir, &domain, port, &uploads_dir, &theme_dir)?;
    write_systemd_service(output_dir, &install_dir)?;

    // ── Summary ────────────────────────────────────────────────────────────

    println!("\n── Done ─────────────────────────────────────────────────");

    // Warn if the app is already running — its site cache needs a restart to
    // reflect the newly created site and admin account.
    let pid_file = std::path::Path::new(&install_dir).join(".synaptic.pid");
    if pid_file.exists() {
        println!("\n⚠️  The app is currently running.");
        println!("   Run './app.sh rebuild' to restart it and load the new site into memory.");
    } else {
        println!("\n   Run './app.sh rebuild' to build and start the app.");
    }

    println!("\nNext steps:");
    println!(
        "  1. Copy the binary and files to {}",
        install_dir
    );
    println!("  2. Copy {} to /etc/caddy/Caddyfile (or include it)",
        output_dir.join("Caddyfile").display()
    );
    println!("     Then run: caddy reload");
    println!("  3. Copy {} to /etc/systemd/system/",
        output_dir.join("synaptic-signals.service").display()
    );
    println!("     Then run: systemctl daemon-reload && systemctl enable --now synaptic-signals");
    println!("  4. Create {install_dir}/.env with your DATABASE_URL and SECRET_KEY");
    println!("\nSite will be live at: https://{}", domain);

    Ok(())
}

fn write_caddyfile(
    output_dir: &std::path::Path,
    domain: &str,
    port: u16,
    uploads_dir: &str,
    theme_dir: &str,
) -> anyhow::Result<()> {
    // Try to read the template from the deployment directory relative to CWD
    let template = find_template("deployment/Caddyfile.template")
        .unwrap_or_else(|| include_str!("../../deployment_templates/Caddyfile.template").to_string());

    let content = template
        .replace("{DOMAIN}", domain)
        .replace("{PORT}", &port.to_string())
        .replace("{UPLOADS_DIR}", uploads_dir)
        .replace("{THEME_DIR}", theme_dir);

    let path = output_dir.join("Caddyfile");
    std::fs::write(&path, content)
        .map_err(|e| anyhow::anyhow!("Failed to write Caddyfile: {e}"))?;
    println!("Written: {}", path.display());
    Ok(())
}

fn write_systemd_service(output_dir: &std::path::Path, install_dir: &str) -> anyhow::Result<()> {
    let template = find_template("deployment/synaptic-signals.service")
        .unwrap_or_else(|| include_str!("../../deployment_templates/synaptic-signals.service").to_string());

    let content = template.replace("{INSTALL_DIR}", install_dir);

    let path = output_dir.join("synaptic-signals.service");
    std::fs::write(&path, content)
        .map_err(|e| anyhow::anyhow!("Failed to write service file: {e}"))?;
    println!("Written: {}", path.display());
    Ok(())
}

/// Try to read a template file from the filesystem (relative to CWD).
/// Returns None if the file doesn't exist.
fn find_template(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn hash_password(password: &str) -> anyhow::Result<String> {
    use argon2::{password_hash::{rand_core::OsRng, PasswordHasher, SaltString}, Argon2};
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("Password hashing failed: {e}"))
}
