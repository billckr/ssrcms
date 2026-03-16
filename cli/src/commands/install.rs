use clap::Args;
use dialoguer::{Confirm, Input, Password};
use uuid::Uuid;

#[derive(Args)]
pub struct InstallArgs {
    /// Skip interactive prompts — reads all values from flags or env vars.
    /// Required env vars in non-interactive mode: SYNAPTIC_DOMAIN, ADMIN_EMAIL.
    /// ADMIN_PASSWORD is optional; a compliant password is generated if omitted.
    #[arg(long)]
    pub non_interactive: bool,

    /// Output directory for Caddyfile and .service (defaults to current dir)
    #[arg(long, default_value = ".")]
    pub output_dir: String,

    // ── Non-interactive / env-var-backed fields ───────────────────────────
    /// Domain name (e.g. example.com). Env: SYNAPTIC_DOMAIN
    #[arg(long, env = "SYNAPTIC_DOMAIN")]
    pub domain: Option<String>,

    /// Port Axum listens on. Env: PORT
    #[arg(long, env = "PORT", default_value = "3000")]
    pub port: u16,

    /// Install directory (full path). Env: INSTALL_DIR
    #[arg(long, env = "INSTALL_DIR")]
    pub install_dir: Option<String>,

    /// Admin login email. Env: ADMIN_EMAIL
    #[arg(long, env = "ADMIN_EMAIL")]
    pub admin_email: Option<String>,

    /// Admin username. Env: ADMIN_USERNAME
    #[arg(long, env = "ADMIN_USERNAME")]
    pub admin_username: Option<String>,

    /// Admin display name. Env: ADMIN_DISPLAY_NAME
    #[arg(long, env = "ADMIN_DISPLAY_NAME")]
    pub admin_display_name: Option<String>,

    /// Admin password (must satisfy policy). Env: ADMIN_PASSWORD
    /// If omitted in non-interactive mode a compliant password is generated and printed once.
    #[arg(long, env = "ADMIN_PASSWORD")]
    pub admin_password: Option<String>,

    /// System notification / reply-to email. Env: NOTIFICATION_EMAIL
    #[arg(long, env = "NOTIFICATION_EMAIL")]
    pub notification_email: Option<String>,

    /// Admin panel brand name. Env: APP_NAME
    #[arg(long, env = "APP_NAME")]
    pub app_name: Option<String>,

    /// System user the app runs as (e.g. www-data, synaptic).
    /// When provided, the installer sets up Caddy write permissions and the
    /// sudoers entry needed for SSL provisioning from the admin panel.
    /// Requires root. Env: APP_USER
    #[arg(long, env = "APP_USER")]
    pub app_user: Option<String>,
}

/// Returns the current effective UID.
#[cfg(unix)]
fn current_uid() -> u32 {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("Uid:") {
                if let Some(uid_str) = line.split_whitespace().nth(1) {
                    if let Ok(uid) = uid_str.parse::<u32>() {
                        return uid;
                    }
                }
            }
        }
    }
    0
}

/// Returns the current username from the USER env var.
fn current_username() -> String {
    std::env::var("USER").unwrap_or_else(|_| "unknown".to_string())
}

pub async fn run(args: InstallArgs) -> anyhow::Result<()> {
    println!("\nWelcome to Synaptic Signals CMS Installer");
    println!("==========================================\n");

    let ni = args.non_interactive;

    // ── Gather configuration ───────────────────────────────────────────────

    let domain: String = prompt_or(ni, args.domain, || {
        Input::new()
            .with_prompt("Domain name (e.g. example.com)")
            .interact_text()
            .map_err(Into::into)
    })?;

    let port: u16 = if ni {
        args.port
    } else {
        Input::new()
            .with_prompt("Port Axum listens on")
            .default(args.port)
            .interact_text()?
    };

    let install_dir: String = prompt_or(ni, args.install_dir, || {
        Input::new()
            .with_prompt("Install directory (full path)")
            .default(
                std::env::current_dir()
                    .ok()
                    .and_then(|p| p.to_str().map(String::from))
                    .unwrap_or_else(|| "/opt/synaptic-signals".to_string()),
            )
            .interact_text()
            .map_err(Into::into)
    })?;

    // ── Install dir ownership check ───────────────────────────────────────
    // If the directory already exists it must be owned by the current user.
    let service_user = current_username();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let dir_path = std::path::Path::new(&install_dir);
        if dir_path.exists() {
            match std::fs::metadata(dir_path) {
                Ok(meta) => {
                    let dir_uid  = meta.uid();
                    let my_uid   = current_uid();
                    if dir_uid != my_uid {
                        eprintln!("Error: {} is not owned by the current user ({}).",
                            install_dir, service_user);
                        eprintln!("  Directory owner uid : {}", dir_uid);
                        eprintln!("  Your uid            : {}", my_uid);
                        eprintln!();
                        eprintln!("Fix ownership before installing:");
                        eprintln!("  sudo chown -R {}:{} {}", service_user, service_user, install_dir);
                        anyhow::bail!("Installation cancelled — fix directory ownership first.");
                    }
                }
                Err(e) => {
                    eprintln!("Warning: could not stat {} — {}", install_dir, e);
                }
            }
        }
    }

    let database_url: String = if ni {
        std::env::var("DATABASE_URL").map_err(|_| {
            anyhow::anyhow!("DATABASE_URL env var is required in --non-interactive mode")
        })?
    } else {
        Input::new()
            .with_prompt("Database URL")
            .default(
                std::env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "postgres://synaptic:password@localhost:5432/synaptic_signals".to_string()),
            )
            .interact_text()?
    };

    println!("\n── Database ─────────────────────────────────────────────");
    println!("Connecting to database...");

    // SAFETY: single-threaded at this point in the installer; no other threads read env.
    unsafe { std::env::set_var("DATABASE_URL", &database_url); }
    let pool = super::connect_db().await?;

    println!("Running migrations...");
    sqlx::migrate!("../migrations")
        .run(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Migration failed: {e}"))?;
    println!("Migrations applied.\n");

    // ── Admin user ─────────────────────────────────────────────────────────

    let create_admin = if ni {
        // In non-interactive mode: create admin iff ADMIN_EMAIL is provided.
        args.admin_email.is_some()
    } else {
        Confirm::new()
            .with_prompt("Create an admin user now?")
            .default(true)
            .interact()?
    };

    let mut admin_id: Option<Uuid> = None;
    let mut notification_email: Option<String> = args.notification_email.clone();

    if create_admin {
        println!("\n── Admin User ───────────────────────────────────────────");

        let username: String = prompt_or(ni, args.admin_username.clone(), || {
            Input::new()
                .with_prompt("Admin username")
                .default("admin".to_string())
                .interact_text()
                .map_err(Into::into)
        })?;

        let email: String = prompt_or(ni, args.admin_email.clone(), || {
            Input::new()
                .with_prompt("Admin login email")
                .interact_text()
                .map_err(Into::into)
        })?;

        if notification_email.is_none() {
            notification_email = Some(if ni {
                email.clone()
            } else {
                Input::new()
                    .with_prompt("System notification email (reply-to for outbound mail)")
                    .default(email.clone())
                    .interact_text()?
            });
        }

        let display_name: String = if ni {
            args.admin_display_name.clone().unwrap_or_else(|| username.clone())
        } else {
            Input::new()
                .with_prompt("Display name")
                .default(username.clone())
                .interact_text()?
        };

        // Password: use provided value, generate one, or prompt interactively.
        let password = if ni {
            match args.admin_password.clone() {
                Some(pw) => {
                    validate_password(&pw).map_err(|e| anyhow::anyhow!("Provided ADMIN_PASSWORD is invalid: {e}"))?;
                    pw
                }
                None => {
                    let pw = generate_password();
                    println!("GENERATED_ADMIN_PASSWORD={pw}");
                    println!("IMPORTANT: Save this password — it will not be shown again.");
                    pw
                }
            }
        } else {
            loop {
                let pw = Password::new()
                    .with_prompt("Admin password (8-12 chars, 1 uppercase, 1 number, 1 symbol: !@#$%&)")
                    .with_confirmation("Confirm password", "Passwords do not match")
                    .interact()?;
                match validate_password(&pw) {
                    Ok(()) => break pw,
                    Err(msg) => eprintln!("Password error: {msg}"),
                }
            }
        };

        let hash = hash_password(&password)?;
        let id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO users (id, username, email, display_name, password_hash, role, is_protected, created_at)
             VALUES ($1, $2, $3, $4, $5, 'super_admin', TRUE, NOW())
             ON CONFLICT (email) DO UPDATE SET password_hash = EXCLUDED.password_hash, updated_at = NOW()"
        )
        .bind(id)
        .bind(&username)
        .bind(&email)
        .bind(&display_name)
        .bind(&hash)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create admin user: {e}"))?;

        // Fetch the actual ID — the user may have already existed (ON CONFLICT DO NOTHING),
        // in which case `id` above was never inserted and would break FK constraints.
        let actual_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
            .bind(&email)
            .fetch_one(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to look up admin user: {e}"))?;
        admin_id = Some(actual_id);

        println!("Admin user '{}' ({}) created.", username, email);
    }

    // ── Initial site ───────────────────────────────────────────────────────
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

    let site_url = match port {
        80  => format!("http://{domain}"),
        443 => format!("https://{domain}"),
        _   => format!("http://{domain}:{port}"),
    };
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

    // ── Branding ───────────────────────────────────────────────────────────
    println!("\n── Branding ─────────────────────────────────────────────");

    let app_name: String = prompt_or(ni, args.app_name.clone(), || {
        Input::new()
            .with_prompt("Admin panel name (shown in the sidebar)")
            .default("My App".to_string())
            .interact_text()
            .map_err(Into::into)
    })?;

    for (key, value) in &[
        ("app_name",      app_name.as_str()),
        ("timezone",      "UTC"),
        ("max_upload_mb", "25"),
    ] {
        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES ($1, $2)
             ON CONFLICT (key) DO NOTHING"
        )
        .bind(key)
        .bind(value)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to seed app_settings: {e}"))?;
    }

    // Create the site's data directories and seed the default theme.
    let site_themes_dst = std::path::Path::new(&install_dir)
        .join("sites").join(site_id.to_string()).join("themes").join("default");
    let site_uploads_dst = std::path::Path::new(&install_dir)
        .join("uploads").join(site_id.to_string());
    let _ = std::fs::create_dir_all(&site_uploads_dst);

    let theme_src = std::path::Path::new(&install_dir)
        .join("themes").join("global").join("default");
    if theme_src.is_dir() {
        match copy_dir_all(&theme_src, &site_themes_dst) {
            Ok(()) => {}
            Err(e) => println!(
                "Warning: could not copy default theme ({}). \
                 Copy themes/global/default/ to sites/{}/themes/default/ manually.",
                e, site_id
            ),
        }
    } else {
        println!(
            "Note: themes/global/default/ not found at '{}'. \
             Copy it to sites/{}/themes/default/ after placing the themes directory.",
            theme_src.display(), site_id
        );
    }

    // Set the super admin's default site (controls home site and visiting badge).
    // No site_users row is needed — global admins have full access on every site
    // via the middleware and do not need a site_users entry.
    if let Some(uid) = admin_id {
        sqlx::query(
            "UPDATE users SET default_site_id = $1, updated_at = NOW() WHERE id = $2 AND default_site_id IS NULL"
        )
        .bind(site_id)
        .bind(uid)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set default site: {e}"))?;
    }

    // ── Deployment files ───────────────────────────────────────────────────
    let uploads_dir = format!("{}/uploads", install_dir);
    let theme_dir   = format!("{}/themes", install_dir);
    let output_dir  = std::path::Path::new(&args.output_dir);

    println!("\n── Deployment Files ─────────────────────────────────────");

    write_caddyfile(output_dir, &domain, port, &uploads_dir, &theme_dir)?;
    write_systemd_service(output_dir, &install_dir, &service_user)?;

    // ── Caddy permissions (SSL provisioning from admin panel) ──────────────
    // Resolve app_user: flag → env → interactive prompt (skippable).
    let app_user: Option<String> = if let Some(u) = args.app_user {
        Some(u)
    } else if ni {
        None  // non-interactive without --app-user: skip silently, print note later
    } else {
        let val: String = Input::new()
            .with_prompt(
                "App system user for Caddy SSL permissions (e.g. www-data) \
                 [leave blank to skip]"
            )
            .allow_empty(true)
            .interact_text()?;
        let trimmed = val.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    };

    if let Some(ref user) = app_user {
        println!("\n── Caddy SSL Permissions ────────────────────────────────");
        let caddyfile_live = "/etc/caddy/Caddyfile";
        match super::caddy::setup_caddy_permissions(user, caddyfile_live) {
            Ok(()) => println!("  SSL provisioning permissions configured."),
            Err(e) => {
                println!("  Warning: could not set up Caddy permissions ({e}).");
                println!(
                    "  Re-run as root when ready:  \
                     sudo synap-cli caddy setup --app-user {}",
                    user
                );
            }
        }
    }

    // ── Write / update .env ────────────────────────────────────────────────
    let env_path = std::path::Path::new(&install_dir).join(".env");
    write_env_key(&env_path, "INSTALL_DIR", &install_dir);
    write_env_key(&env_path, "MAX_UPLOAD_MB", "25");
    if let Some(ref ae) = notification_email {
        write_env_key(&env_path, "ADMIN_EMAIL", ae);
    }

    // ── Install Summary ────────────────────────────────────────────────────
    println!("\n── Installation Summary ─────────────────────────────────");
    println!("  App name    : {}", app_name);
    println!("  Service user: {}", service_user);
    println!("  Site name   : {}", domain);
    println!("  Domain      : {}", domain);
    println!("  Install dir : {}", install_dir);
    if admin_id.is_some() {
        println!("  Admin user  : seeded (see credentials you entered above)");
    }
    println!("  Site URL    : {}", site_url);

    // In non-interactive mode the install script handles deployment — skip the manual steps.
    if !ni {
        let pid_file = std::path::Path::new(&install_dir).join(".synaptic.pid");
        let app_sh   = std::path::Path::new(&install_dir).join("app.sh");

        println!("\n── Next Steps ───────────────────────────────────────────");

        if app_sh.exists() {
            // Dev environment: app.sh is present — start or rebuild automatically.
            let is_running = pid_file.exists() && {
                std::fs::read_to_string(&pid_file)
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .map(|pid| {
                        std::process::Command::new("kill")
                            .args(["-0", &pid.to_string()])
                            .status()
                            .map(|s| s.success())
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
            };

            let action = if is_running { "rebuild" } else { "start" };
            println!("  Running ./app.sh {}...\n", action);
            let status = std::process::Command::new("bash")
                .arg(&app_sh)
                .arg(action)
                .current_dir(&install_dir)
                .status();
            match status {
                Ok(s) if s.success() => {}
                Ok(s) => println!("  Warning: app.sh {} exited with status {}", action, s),
                Err(e) => println!("  Warning: could not run app.sh {}: {}", action, e),
            }
        } else {
            // Systemd / production deployment: show manual steps.
            println!("  1. Copy the binary and files to {}", install_dir);
            println!("  2. Copy {} to /etc/systemd/system/",
                output_dir.join("synaptic-signals.service").display()
            );
            println!("  3. Copy {} to /etc/caddy/Caddyfile (or include it)",
                output_dir.join("Caddyfile").display()
            );
            println!("     Then run: sudo caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile");
            println!("  4. Run:  sudo synap-cli caddy setup --app-user {}", service_user);
            println!("     Sets up Caddy write permissions + log directory for SSL provisioning.");
            println!("  5. Ensure {install_dir}/.env contains DATABASE_URL and SECRET_KEY");
            println!("     (INSTALL_DIR has been written automatically)");
            println!("  6. Run:  systemctl daemon-reload && systemctl enable --now synaptic-signals");
        }

        println!("\nSite will be live at: https://{}", domain);
    }

    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// In non-interactive mode, return the provided value (error if missing and required).
/// In interactive mode, run the dialoguer closure.
fn prompt_or<T, F>(non_interactive: bool, provided: Option<T>, interactive: F) -> anyhow::Result<T>
where
    F: FnOnce() -> anyhow::Result<T>,
{
    if let Some(val) = provided {
        return Ok(val);
    }
    if non_interactive {
        return Err(anyhow::anyhow!(
            "Required value missing in --non-interactive mode. \
             Pass it as a CLI flag or environment variable."
        ));
    }
    interactive()
}

/// Generate a password that satisfies validate_password():
/// 8-12 chars, ≥1 uppercase, ≥1 digit, ≥1 symbol from !@#$%&
fn generate_password() -> String {
    use rand::seq::SliceRandom;
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let lower:   Vec<char> = ('a'..='z').collect();
    let upper:   Vec<char> = ('A'..='Z').collect();
    let digits:  Vec<char> = ('0'..='9').collect();
    // Exclude $ and ! — they get mangled in shell env vars and URL strings.
    let symbols: &[char]   = &['@', '#', '%', '&'];

    // Guarantee one of each required class within the 10-char budget.
    let mut chars: Vec<char> = Vec::with_capacity(10);
    chars.push(upper[rng.gen_range(0..upper.len())]);
    chars.push(digits[rng.gen_range(0..digits.len())]);
    chars.push(symbols[rng.gen_range(0..symbols.len())]);
    // Fill remaining 7 slots with lowercase.
    for _ in 0..7 {
        chars.push(lower[rng.gen_range(0..lower.len())]);
    }
    chars.shuffle(&mut rng);
    chars.into_iter().collect()
}

fn write_caddyfile(
    output_dir: &std::path::Path,
    domain: &str,
    port: u16,
    uploads_dir: &str,
    theme_dir: &str,
) -> anyhow::Result<()> {
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

fn write_systemd_service(output_dir: &std::path::Path, install_dir: &str, service_user: &str) -> anyhow::Result<()> {
    let template = find_template("deployment/synaptic-signals.service")
        .unwrap_or_else(|| include_str!("../../deployment_templates/synaptic-signals.service").to_string());

    let content = template
        .replace("{INSTALL_DIR}", install_dir)
        .replace("{SERVICE_USER}", service_user);

    let path = output_dir.join("synaptic-signals.service");
    std::fs::write(&path, content)
        .map_err(|e| anyhow::anyhow!("Failed to write service file: {e}"))?;
    println!("Written: {}", path.display());
    Ok(())
}

fn find_template(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn validate_password(password: &str) -> Result<(), &'static str> {
    let len = password.len();
    if len < 8 {
        return Err("Password must be at least 8 characters");
    }
    if len > 12 {
        return Err("Password must be no more than 12 characters");
    }
    if !password.chars().any(|c| c.is_uppercase()) {
        return Err("Password must contain at least one uppercase letter");
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err("Password must contain at least one number");
    }
    const ALLOWED_SYMBOLS: &[char] = &['!', '@', '#', '$', '%', '&', '*', '-', '_', '+'];
    if !password.chars().any(|c| ALLOWED_SYMBOLS.contains(&c)) {
        return Err("Password must contain at least one symbol: ! @ # $ % &");
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

fn hash_password(password: &str) -> anyhow::Result<String> {
    use argon2::{password_hash::{rand_core::OsRng, PasswordHasher, SaltString}, Argon2};
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("Password hashing failed: {e}"))
}

/// Write (or update) a single `KEY=value` line in a .env file.
fn write_env_key(path: &std::path::Path, key: &str, value: &str) {
    let line = format!("{}={}", key, value);
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let prefix = format!("{}=", key);

    let updated: String = if existing.lines().any(|l| l.starts_with(&prefix)) {
        existing.lines()
            .map(|l| if l.starts_with(&prefix) { line.as_str() } else { l })
            .collect::<Vec<_>>()
            .join("\n") + "\n"
    } else {
        if existing.is_empty() {
            format!("{line}\n")
        } else if existing.ends_with('\n') {
            format!("{existing}{line}\n")
        } else {
            format!("{existing}\n{line}\n")
        }
    };

    if let Err(e) = std::fs::write(path, &updated) {
        println!(
            "Warning: could not write {}={} to {} ({}). \
             Add it manually.",
            key, value, path.display(), e
        );
    }
}
