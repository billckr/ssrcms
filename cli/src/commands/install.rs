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

    /// Public-facing site URL, e.g. https://example.com. Env: SITE_URL
    /// Overrides the default derivation from domain+port. Set this whenever
    /// a reverse proxy (Caddy) fronts the app on a different port than
    /// Axum's own listen port — the default derivation otherwise bakes the
    /// internal port (e.g. :3000) into permalinks, which breaks external
    /// links since the public port is actually 443/none.
    #[arg(long, env = "SITE_URL")]
    pub site_url: Option<String>,

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

    /// Create a local Postgres role/database before connecting, instead of
    /// requiring an already-working DATABASE_URL. Off by default — only
    /// relevant when no DATABASE_URL is already usable. Env: SYNAP_BOOTSTRAP_DB
    #[arg(long, env = "SYNAP_BOOTSTRAP_DB")]
    pub bootstrap_db: bool,

    /// Postgres role to create/use when bootstrapping a local database. Env: DB_USER
    #[arg(long, env = "DB_USER", default_value = "synaptic")]
    pub db_user: String,

    /// Postgres database name to create/use when bootstrapping a local database. Env: DB_NAME
    #[arg(long, env = "DB_NAME", default_value = "synaptic_signals")]
    pub db_name: String,

    /// Postgres password for db_user. Env: DB_PASSWORD
    /// If omitted, a random password is generated when bootstrapping.
    #[arg(long, env = "DB_PASSWORD")]
    pub db_password: Option<String>,

    /// Copy the generated Caddyfile/systemd unit into place and enable the
    /// service locally (requires sudo). Off by default — this touches
    /// system-wide Caddy/systemd config and can conflict with an
    /// already-running dev instance on the same port. Env: SYNAP_SETUP_SERVICE
    #[arg(long, env = "SYNAP_SETUP_SERVICE")]
    pub setup_service: bool,

    /// Path to the built `synaptic` binary to install as {install_dir}/synaptic
    /// when --setup-service is used. Defaults to target/release/synaptic (then
    /// target/debug/synaptic) relative to the current directory.
    #[arg(long)]
    pub synaptic_bin: Option<String>,

    /// Path to the built `synap` CLI binary to install as {install_dir}/synap
    /// when --setup-service is used. Defaults similarly to synaptic_bin.
    #[arg(long)]
    pub synap_bin: Option<String>,
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

    // Check for an already-working DATABASE_URL — the process env (today's
    // only source) or install_dir/.env (covers a dev machine where the URL
    // lives only in a project .env, not the shell environment). If found,
    // never offer/require bootstrap — just use it (or let it be edited).
    let existing_db_url = std::env::var("DATABASE_URL").ok()
        .or_else(|| read_env_key(&std::path::Path::new(&install_dir).join(".env"), "DATABASE_URL"));

    let database_url: String = if ni {
        match (existing_db_url, args.bootstrap_db) {
            (Some(url), _) => url,
            (None, true) => bootstrap_local_db(&args.db_user, &args.db_name, args.db_password.clone())?,
            (None, false) => return Err(anyhow::anyhow!(
                "DATABASE_URL env var is required in --non-interactive mode \
                 (or pass --bootstrap-db to create a local database)."
            )),
        }
    } else if let Some(url) = existing_db_url {
        Input::new()
            .with_prompt("Database URL")
            .default(url)
            .interact_text()?
    } else {
        let want_bootstrap = Confirm::new()
            .with_prompt(
                "No DATABASE_URL found. Create a local Postgres role/database now? \
                 (requires sudo access to run commands as the 'postgres' user)"
            )
            .default(false)
            .interact()?;
        if want_bootstrap {
            bootstrap_local_db(&args.db_user, &args.db_name, args.db_password.clone())?
        } else {
            Input::new()
                .with_prompt("Database URL")
                .default("postgres://synaptic:password@localhost:5432/synaptic_signals".to_string())
                .interact_text()?
        }
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
    sqlx::query(
        "INSERT INTO sites (id, hostname, owner_user_id, created_at, updated_at)
         VALUES ($1, $2, $3, NOW(), NOW())
         ON CONFLICT (hostname) DO NOTHING"
    )
    .bind(Uuid::new_v4())
    .bind(&domain)
    .bind(admin_id)
    .execute(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create initial site: {e}"))?;

    // Fetch the actual ID — on a re-run the site may already exist (ON CONFLICT
    // DO NOTHING), in which case the freshly generated UUID above was never
    // inserted and would break the FK constraints below.
    let site_id: Uuid = sqlx::query_scalar("SELECT id FROM sites WHERE hostname = $1")
        .bind(&domain)
        .fetch_one(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to look up site: {e}"))?;

    let derived_site_url = match port {
        80  => format!("http://{domain}"),
        443 => format!("https://{domain}"),
        _   => format!("http://{domain}:{port}"),
    };
    let site_url = if ni {
        args.site_url.clone().unwrap_or(derived_site_url)
    } else {
        Input::new()
            .with_prompt(
                "Public site URL (the address visitors actually use — if a reverse \
                 proxy like Caddy fronts this on 443, that's https://domain with NO \
                 port, even though Axum itself listens on the port above)"
            )
            .default(args.site_url.clone().unwrap_or(derived_site_url))
            .interact_text()?
    };
    let settings_defaults: &[(&str, &str)] = &[
        ("site_name",        &domain),
        ("site_description", ""),
        ("site_url",         &site_url),
        ("site_language",    "en-US"),
        ("active_theme",     "default"),
        ("posts_per_page",   "9"),
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

    // ── Local service setup (opt-in) ────────────────────────────────────────
    // Decided here (needs `port` for the warning text) but actually performed
    // further down, after .env is finalized — the systemd unit reads
    // {install_dir}/.env on start, so it must be complete first.
    let do_setup_service = if args.setup_service {
        true
    } else if ni {
        false // non-interactive without the explicit flag: never touch system state
    } else {
        println!();
        println!("Optional: install this as a local systemd service fronted by Caddy.");
        println!("  This copies files into /etc/caddy/ and /etc/systemd/system/, reloads");
        println!("  Caddy, and enables+starts a synapcms.service. It requires sudo,");
        println!("  and if a dev instance is already running (e.g. via ./app.sh on port");
        println!("  {port}), a systemd-managed instance could try to bind the same port");
        println!("  and conflict with it.");
        Confirm::new()
            .with_prompt("Set up Caddy + systemd service now?")
            .default(false)
            .interact()?
    };

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
                     sudo synap caddy setup --app-user {}",
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

    // These must never clobber an existing working .env, but a from-scratch
    // install run alone (no wrapper script) needs them written at least once,
    // or the app has no DATABASE_URL / falls back to the insecure default
    // SECRET_KEY. No-ops when the key already exists (e.g. install-vps.sh
    // already wrote a complete .env before invoking --non-interactive).
    write_env_key_if_absent(&env_path, "DATABASE_URL", &database_url);
    write_env_key_if_absent(&env_path, "SECRET_KEY", &generate_secret_key());
    write_env_key_if_absent(&env_path, "HOST", "0.0.0.0");
    write_env_key_if_absent(&env_path, "PORT", &port.to_string());
    write_env_key_if_absent(&env_path, "LOG_LEVEL", "info");

    if do_setup_service {
        setup_local_service(&install_dir, output_dir, args.synaptic_bin.as_deref(), args.synap_bin.as_deref())?;
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
    if do_setup_service {
        println!("  Local service: enabled and started (systemctl status synapcms)");
    }

    // The running server (if any) loaded its site cache at startup and does not
    // watch the database for new sites — a restart is required to pick this one
    // up, even though the DB write above already succeeded. Print this
    // unconditionally (interactive and --non-interactive) since it's easy to
    // miss otherwise: no errors are shown, but the homepage 404s with
    // "No site found for hostname" until the service is restarted.
    println!();
    println!("  IMPORTANT: if synapcms is already running, restart it now");
    println!("  so it picks up this site — the DB write above does not take effect");
    println!("  on a running server until it restarts:");
    println!("    systemctl restart synapcms");

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
            // Systemd / production deployment. Check whether this is a genuinely
            // fresh install or a re-run against an already-deployed service —
            // printing the full "copy these files, enable the service" checklist
            // every time is actively misleading on a re-run: it reads like
            // required steps even when everything is already in place and the
            // only real action needed is restarting to pick up this run's
            // database changes (see the restart notice above).
            let live_unit_path = std::path::Path::new("/etc/systemd/system/synapcms.service");
            let live_caddy_path = std::path::Path::new("/etc/caddy/Caddyfile");
            let generated_unit = output_dir.join("synapcms.service");
            let generated_caddy = output_dir.join("Caddyfile");

            if live_unit_path.exists() {
                let unit_matches = std::fs::read(&generated_unit).ok() == std::fs::read(live_unit_path).ok();
                let caddy_matches = std::fs::read(&generated_caddy).ok() == std::fs::read(live_caddy_path).ok();

                if unit_matches && caddy_matches {
                    println!("  systemd unit and Caddyfile already match what's live — nothing to copy.");
                } else {
                    println!("  This run changed the generated systemd unit and/or Caddyfile");
                    println!("  (e.g. domain, port, or service user) — re-apply the changed one(s):");
                    if !unit_matches {
                        println!("    cp {} /etc/systemd/system/ && systemctl daemon-reload",
                            generated_unit.display());
                    }
                    if !caddy_matches {
                        println!("    cp {} /etc/caddy/Caddyfile && caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile",
                            generated_caddy.display());
                    }
                }
            } else {
                // No unit installed yet — this really is a fresh systemd deployment.
                println!("  1. Copy the binary and files to {}", install_dir);
                println!("  2. Copy {} to /etc/systemd/system/", generated_unit.display());
                println!("  3. Copy {} to /etc/caddy/Caddyfile (or include it)", generated_caddy.display());
                println!("     Then run: sudo caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile");
                println!("  4. Run:  sudo synap caddy setup --app-user {}", service_user);
                println!("     Sets up Caddy write permissions + log directory for SSL provisioning.");
                println!("  5. Ensure {install_dir}/.env contains DATABASE_URL and SECRET_KEY");
                println!("     (INSTALL_DIR has been written automatically)");
                println!("  6. Run:  systemctl daemon-reload && systemctl enable --now synapcms");
            }
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

/// Create (idempotently) a local Postgres role + database via
/// `sudo -u postgres psql`, mirroring install-vps.sh's do_db_bootstrap but
/// run directly on this machine (no ssh). Returns the resulting DATABASE_URL.
fn bootstrap_local_db(db_user: &str, db_name: &str, db_password: Option<String>) -> anyhow::Result<String> {
    println!("\n── Local Database Bootstrap ─────────────────────────────");
    let password = db_password.unwrap_or_else(generate_db_password);

    let sql = format!(
        r#"DO $$ BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = '{db_user}') THEN
    EXECUTE format('CREATE ROLE {db_user} LOGIN PASSWORD %L', '{password}');
  ELSE
    EXECUTE format('ALTER ROLE {db_user} WITH LOGIN PASSWORD %L', '{password}');
  END IF;
END $$;
SELECT 'CREATE DATABASE {db_name} OWNER {db_user}'
  WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = '{db_name}') \gexec
GRANT ALL PRIVILEGES ON DATABASE {db_name} TO {db_user};
"#
    );

    // Validate/cache the sudo timestamp first, with inherited stdio, so any
    // password prompt goes to the real tty. The actual psql call below pipes
    // the SQL over stdin, which would otherwise fight with sudo's own prompt.
    println!("You may be prompted for your sudo password...");
    let sudo_ok = std::process::Command::new("sudo")
        .arg("-v")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !sudo_ok {
        anyhow::bail!(
            "sudo access is required to bootstrap a local database (sudo -v failed). \
             Bootstrap the database manually and re-run with DATABASE_URL set, or omit --bootstrap-db."
        );
    }

    let status = std::process::Command::new("sudo")
        .args(["-u", "postgres", "psql", "-v", "ON_ERROR_STOP=1"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(sql.as_bytes())?;
            child.wait()
        });

    match status {
        Ok(s) if s.success() => println!("Database role/database ready."),
        Ok(s) => anyhow::bail!(
            "sudo -u postgres psql exited with {s}. Common causes: PostgreSQL not \
             installed locally, or the 'postgres' OS user not present. Bootstrap the \
             database manually and re-run with DATABASE_URL set, or omit --bootstrap-db."
        ),
        Err(e) => anyhow::bail!(
            "Failed to run `sudo -u postgres psql`: {e}. Is sudo installed and is \
             PostgreSQL installed locally? Bootstrap manually and re-run with \
             DATABASE_URL set."
        ),
    }

    Ok(format!("postgres://{db_user}:{password}@localhost:5432/{db_name}"))
}

/// Generate a random hex password for a bootstrapped Postgres role.
fn generate_db_password() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Verify systemd/Caddy are actually present locally before attempting to
/// install a service — fail fast with a clear message rather than partway
/// through copying files into /etc/.
fn check_local_service_requirements() -> anyhow::Result<()> {
    let mut problems = Vec::new();
    if std::process::Command::new("systemctl").arg("--version").output().is_err() {
        problems.push("systemctl not found on PATH — systemd is required".to_string());
    }
    if std::process::Command::new("caddy").arg("version").output().is_err() {
        problems.push("caddy not found on PATH".to_string());
    }
    if !std::path::Path::new("/etc/systemd/system").is_dir() {
        problems.push("/etc/systemd/system does not exist".to_string());
    }
    if !problems.is_empty() {
        anyhow::bail!(
            "Cannot set up local service — missing requirements:\n  - {}\n\
             Install systemd/Caddy first, or skip --setup-service.",
            problems.join("\n  - ")
        );
    }
    Ok(())
}

/// Resolve a built binary's path: explicit override, else
/// target/release/{name}, else target/debug/{name}, relative to cwd.
fn resolve_binary(explicit: Option<&str>, name: &str) -> anyhow::Result<std::path::PathBuf> {
    if let Some(p) = explicit {
        let path = std::path::PathBuf::from(p);
        if path.is_file() { return Ok(path); }
        anyhow::bail!("--{name}-bin path '{p}' does not exist or is not a file");
    }
    for candidate in ["target/release", "target/debug"] {
        let path = std::path::PathBuf::from(candidate).join(name);
        if path.is_file() { return Ok(path); }
    }
    anyhow::bail!(
        "Could not find a built '{name}' binary (looked in target/release/{name} \
         and target/debug/{name} relative to the current directory). Run \
         `cargo build --release` first, or pass --{name}-bin <path> explicitly."
    );
}

/// Run `sudo <args>` with inherited stdio (so any password prompt goes to
/// the real tty), bailing with a clear error on failure.
fn run_sudo(args: &[&str]) -> anyhow::Result<()> {
    let status = std::process::Command::new("sudo")
        .args(args)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run `sudo {}`: {e}", args.join(" ")))?;
    if !status.success() {
        anyhow::bail!("`sudo {}` failed ({status})", args.join(" "));
    }
    Ok(())
}

/// If `live_path` already exists, copy it to `{backup_dir}/{label}.bak.<timestamp>`
/// (via sudo, since e.g. /etc/systemd/system/*.service may not be
/// user-readable) before it gets overwritten, and hand ownership of the
/// backup to the invoking user. Returns the backup path if one was made.
/// This machine may already be running a live Caddy/systemd setup fronting
/// other sites — setup_local_service must never clobber that without a
/// way back.
fn backup_if_exists(live_path: &str, backup_dir: &std::path::Path, label: &str) -> anyhow::Result<Option<std::path::PathBuf>> {
    if !std::path::Path::new(live_path).exists() {
        return Ok(None);
    }
    std::fs::create_dir_all(backup_dir)
        .map_err(|e| anyhow::anyhow!("Failed to create backup dir {}: {e}", backup_dir.display()))?;
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_path = backup_dir.join(format!("{label}.bak.{timestamp}"));
    let backup_path_str = backup_path.to_str()
        .ok_or_else(|| anyhow::anyhow!("backup path is not valid UTF-8"))?;
    run_sudo(&["cp", live_path, backup_path_str])?;
    let user = current_username();
    let _ = run_sudo(&["chown", &format!("{user}:{user}"), backup_path_str]);
    println!("  Backed up existing {} -> {}", live_path, backup_path.display());
    Ok(Some(backup_path))
}

/// Local equivalent of install-vps.sh's do_ship_files (binary placement only)
/// + do_caddy_systemd: places the built binaries at {install_dir}/{synaptic,synap},
/// copies the already-generated Caddyfile/service files into place, and
/// enables/starts both Caddy and the synapcms service. No ssh/scp —
/// everything runs directly on this machine, gated by an explicit opt-in.
/// Any live Caddyfile/systemd unit this would overwrite is backed up first
/// (see `backup_if_exists`) — this machine may already be running a
/// different site through the same files.
fn setup_local_service(
    install_dir: &str,
    output_dir: &std::path::Path,
    synaptic_bin_arg: Option<&str>,
    synap_bin_arg: Option<&str>,
) -> anyhow::Result<()> {
    println!("\n── Local Service Setup ──────────────────────────────────");
    check_local_service_requirements()?;

    let synaptic_src = resolve_binary(synaptic_bin_arg, "synaptic")?;
    let synap_src    = resolve_binary(synap_bin_arg, "synap")?;
    let synaptic_dst = std::path::Path::new(install_dir).join("synaptic");
    let synap_dst    = std::path::Path::new(install_dir).join("synap");
    std::fs::copy(&synaptic_src, &synaptic_dst)
        .map_err(|e| anyhow::anyhow!("Failed to copy {} -> {}: {e}", synaptic_src.display(), synaptic_dst.display()))?;
    std::fs::copy(&synap_src, &synap_dst)
        .map_err(|e| anyhow::anyhow!("Failed to copy {} -> {}: {e}", synap_src.display(), synap_dst.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for p in [&synaptic_dst, &synap_dst] {
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755));
        }
    }
    println!("  Installed binaries to {}/{{synaptic,synap}}", install_dir);

    let generated_caddy = output_dir.join("Caddyfile");
    let generated_unit  = output_dir.join("synapcms.service");
    let generated_caddy_str = generated_caddy.to_str()
        .ok_or_else(|| anyhow::anyhow!("Caddyfile path is not valid UTF-8"))?;
    let generated_unit_str = generated_unit.to_str()
        .ok_or_else(|| anyhow::anyhow!("service file path is not valid UTF-8"))?;

    let backup_dir = std::path::Path::new(install_dir).join("backups");
    let mut backups_made: Vec<std::path::PathBuf> = Vec::new();

    if let Some(b) = backup_if_exists("/etc/caddy/Caddyfile", &backup_dir, "Caddyfile")? {
        backups_made.push(b);
    }
    run_sudo(&["cp", generated_caddy_str, "/etc/caddy/Caddyfile"])?;
    let caddy_active = std::process::Command::new("systemctl")
        .args(["is-active", "--quiet", "caddy"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if caddy_active {
        run_sudo(&["caddy", "reload", "--config", "/etc/caddy/Caddyfile"])?;
    } else {
        run_sudo(&["systemctl", "enable", "--now", "caddy"])?;
    }

    if let Some(b) = backup_if_exists("/etc/systemd/system/synapcms.service", &backup_dir, "synapcms.service")? {
        backups_made.push(b);
    }
    run_sudo(&["cp", generated_unit_str, "/etc/systemd/system/synapcms.service"])?;
    run_sudo(&["systemctl", "daemon-reload"])?;
    run_sudo(&["systemctl", "enable", "--now", "synapcms"])?;

    println!("  Caddy + systemd service configured and started.");
    if !backups_made.is_empty() {
        println!("\n  Pre-existing files were replaced. To roll back:");
        for b in &backups_made {
            let target = if b.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("Caddyfile")) {
                "/etc/caddy/Caddyfile"
            } else {
                "/etc/systemd/system/synapcms.service"
            };
            println!("    sudo cp {} {}", b.display(), target);
        }
        println!("    sudo systemctl daemon-reload && sudo caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile");
    }
    Ok(())
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
    let template = find_template("deployment/synapcms.service")
        .unwrap_or_else(|| include_str!("../../deployment_templates/synapcms.service").to_string());

    let content = template
        .replace("{INSTALL_DIR}", install_dir)
        .replace("{SERVICE_USER}", service_user);

    let path = output_dir.join("synapcms.service");
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

/// Read a single key's value out of a .env-style file, if present.
fn read_env_key(path: &std::path::Path, key: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let prefix = format!("{}=", key);
    content.lines()
        .find(|l| l.starts_with(&prefix))
        .map(|l| l[prefix.len()..].to_string())
}

/// Write a `KEY=value` line only if that key is not already present in the
/// file. Used for values that must never clobber an existing working .env
/// (e.g. a dev machine's DATABASE_URL/SECRET_KEY) but should still be filled
/// in on a genuinely fresh or partially-configured install.
fn write_env_key_if_absent(path: &std::path::Path, key: &str, value: &str) {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let prefix = format!("{}=", key);
    if existing.lines().any(|l| l.starts_with(&prefix)) {
        return;
    }
    write_env_key(path, key, value);
}

/// 32 random bytes, hex-encoded (64 chars) — same entropy as `openssl rand -hex 32`.
fn generate_secret_key() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
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
