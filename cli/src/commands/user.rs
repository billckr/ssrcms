use clap::Subcommand;
use dialoguer::{Input, Password, Select};
use uuid::Uuid;

#[derive(Subcommand)]
pub enum UserAction {
    /// Create a new user interactively (requires the super-admin password)
    Create {
        /// Super-admin password (skips interactive prompt — use only in scripts).
        #[arg(long)]
        password: Option<String>,
    },
    /// List all users
    List,
    /// Reset a user's password
    ResetPassword,
    /// Hash a password non-interactively and print the Argon2 hash to stdout.
    /// Intended for scripting (e.g. seed scripts) — does not touch the database.
    HashPassword {
        /// Plaintext password to hash
        password: String,
    },
}

pub async fn run(action: UserAction) -> anyhow::Result<()> {
    match action {
        UserAction::Create { password } => create(password).await,
        UserAction::List => list().await,
        UserAction::ResetPassword => reset_password().await,
        UserAction::HashPassword { password } => {
            println!("{}", hash_password(&password)?);
            Ok(())
        }
    }
}

async fn create(admin_password: Option<String>) -> anyhow::Result<()> {
    let pool = super::connect_db().await?;

    // Gate behind the super-admin password before asking for any user
    // details — DB/server access alone shouldn't be enough to mint accounts.
    super::verify_super_admin_password(&pool, admin_password).await?;

    let username: String = loop {
        let candidate: String = Input::new()
            .with_prompt("Username (5-15 chars, lowercase letters, numbers, hyphens)")
            .interact_text()?;
        match validate_username(&candidate) {
            Ok(()) => break candidate,
            Err(msg) => eprintln!("Username error: {msg}"),
        }
    };

    let email: String = Input::new()
        .with_prompt("Email")
        .interact_text()?;

    let display_name: String = loop {
        let candidate: String = Input::new()
            .with_prompt("Display name")
            .default(username.clone())
            .interact_text()?;
        match validate_display_name(&candidate) {
            Ok(()) => break candidate,
            Err(msg) => eprintln!("Display name error: {msg}"),
        }
    };

    let password = loop {
        let pw = Password::new()
            .with_prompt("Password (8-12 chars, 1 uppercase, 1 number, 1 symbol: !@#$%&)")
            .with_confirmation("Confirm password", "Passwords do not match")
            .interact()?;
        match validate_password(&pw) {
            Ok(()) => break pw,
            Err(msg) => eprintln!("Password error: {msg}"),
        }
    };

    let roles = &["super_admin", "editor", "author", "subscriber"];
    let role_idx = Select::new()
        .with_prompt("Role")
        .items(roles)
        .default(0)
        .interact()?;
    let role = roles[role_idx];

    // super_admin has global access and isn't scoped to a site via
    // site_users, so only offer site assignment for the other roles.
    let assigned_site: Option<(Uuid, String)> = if role == "super_admin" {
        None
    } else {
        let sites: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT id, hostname FROM sites ORDER BY created_at"
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list sites: {e}"))?;

        if sites.is_empty() {
            None
        } else {
            let mut options: Vec<String> = vec!["(Unassigned)".to_string()];
            options.extend(sites.iter().map(|(_, hostname)| hostname.clone()));
            let choice_idx = Select::new()
                .with_prompt("Assign to site")
                .items(&options)
                .default(0)
                .interact()?;
            if choice_idx == 0 {
                None
            } else {
                Some(sites[choice_idx - 1].clone())
            }
        }
    };

    // Usernames aren't globally unique anymore (DB has no UNIQUE constraint
    // on users.username) — only within a site's membership, since two
    // independent site owners' accounts shouldn't collide over a username
    // neither knows the other is using. Unassigned users have no site to
    // collide within, so nothing to check. Mirrors
    // synaptic_core::models::user::username_available.
    if let Some((site_id, hostname)) = &assigned_site {
        let taken: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM users u JOIN site_users su ON su.user_id = u.id \
             WHERE u.username = $1 AND su.site_id = $2)"
        )
        .bind(&username)
        .bind(site_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to check username availability: {e}"))?;
        if taken {
            anyhow::bail!("Username '{}' is already taken on site '{}'.", username, hostname);
        }
    }

    // Hash password with Argon2
    let hash = hash_password(&password)?;

    let id = Uuid::new_v4();
    let is_protected = role == "super_admin";
    sqlx::query(
        "INSERT INTO users (id, username, email, display_name, password_hash, role, is_protected, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())"
    )
    .bind(id)
    .bind(&username)
    .bind(&email)
    .bind(&display_name)
    .bind(&hash)
    .bind(role)
    .bind(is_protected)
    .execute(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create user: {e}"))?;

    if let Some((site_id, _)) = &assigned_site {
        // invited_by: NULL — CLI-seeded row, no attributable inviter (matches
        // the convention documented on synaptic_core::models::site_user::add).
        sqlx::query(
            "INSERT INTO site_users (site_id, user_id, role, invited_by)
             VALUES ($1, $2, $3, NULL)
             ON CONFLICT (site_id, user_id, role) DO NOTHING"
        )
        .bind(site_id)
        .bind(id)
        .bind(role)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("User created, but failed to assign site: {e}"))?;
    }

    // Record to audit_log same as the web admin's "create user" flow —
    // there's no logged-in actor here (just the super-admin password gate),
    // so actor_user_id is NULL and actor_email/role are a "cli" sentinel.
    let _ = sqlx::query(
        "INSERT INTO audit_log (actor_user_id, actor_email, actor_role, action, target_type, target_id, target_label, site_id)
         VALUES (NULL, 'cli', 'cli', 'user.created', 'user', $1, $2, $3)"
    )
    .bind(id)
    .bind(&username)
    .bind(assigned_site.as_ref().map(|(sid, _)| *sid))
    .execute(&pool)
    .await;

    println!("\nUser created successfully.");
    println!("  ID:       {}", id);
    println!("  Username: {}", username);
    println!("  Email:    {}", email);
    println!("  Role:     {}", role);
    println!("  Site:     {}", assigned_site.map(|(_, h)| h).unwrap_or_else(|| "Unassigned".to_string()));

    Ok(())
}

async fn list() -> anyhow::Result<()> {
    let pool = super::connect_db().await?;

    let rows = sqlx::query_as::<_, (Uuid, String, String, String, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT id, username, email, role, created_at FROM users ORDER BY created_at"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to list users: {e}"))?;

    if rows.is_empty() {
        println!("No users found.");
        return Ok(());
    }

    println!("\n{:<38} {:<20} {:<30} {:<12} {}", "ID", "Username", "Email", "Role", "Created");
    println!("{}", "-".repeat(115));
    for (id, username, email, role, created_at) in rows {
        let created = created_at
            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        println!("{:<38} {:<20} {:<30} {:<12} {}", id, username, email, role, created);
    }

    Ok(())
}

async fn reset_password() -> anyhow::Result<()> {
    let pool = super::connect_db().await?;

    let email: String = Input::new()
        .with_prompt("User email")
        .interact_text()?;

    let row = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, username FROM users WHERE email = $1"
    )
    .bind(&email)
    .fetch_optional(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("DB error: {e}"))?;

    let (id, username) = match row {
        Some(r) => r,
        None => {
            println!("No user found with email '{}'.", email);
            return Ok(());
        }
    };

    println!("Resetting password for {} ({})", username, id);

    let password = loop {
        let pw = Password::new()
            .with_prompt("New password (8-12 chars, 1 uppercase, 1 number, 1 symbol: !@#$%&)")
            .with_confirmation("Confirm password", "Passwords do not match")
            .interact()?;
        match validate_password(&pw) {
            Ok(()) => break pw,
            Err(msg) => eprintln!("Password error: {msg}"),
        }
    };

    let hash = hash_password(&password)?;

    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&hash)
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update password: {e}"))?;

    println!("Password reset successfully.");
    Ok(())
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
    const ALLOWED_SYMBOLS: &[char] = &['!', '@', '#', '$', '%', '&'];
    if !password.chars().any(|c| ALLOWED_SYMBOLS.contains(&c)) {
        return Err("Password must contain at least one symbol: ! @ # $ % &");
    }
    Ok(())
}

/// Mirrors `synaptic_core::models::user::RESERVED_USERNAMES`. Keep in sync.
const RESERVED_USERNAMES: &[&str] = &[
    "admin", "administrator", "root", "superuser", "sysadmin", "system",
    "support", "help", "helpdesk", "webmaster", "postmaster", "hostmaster",
    "info", "contact", "sales", "billing", "security", "abuse", "noreply",
    "no-reply", "mail", "email", "ftp", "www", "api", "null", "undefined",
    "test", "guest", "anonymous", "moderator", "mod", "staff", "owner",
    "service", "bot", "official", "synapcms",
];

/// Mirrors `synaptic_core::models::user::validate_username` — duplicated
/// rather than imported (same reasoning as `validate_password` above: the
/// CLI doesn't depend on the core crate). Keep in sync if the web rule changes.
fn validate_username(username: &str) -> Result<(), &'static str> {
    let len = username.len();
    if len < 5 {
        return Err("Username must be at least 5 characters");
    }
    if len > 15 {
        return Err("Username must be no more than 15 characters");
    }
    if !username.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err("Username may only contain lowercase letters, numbers and hyphens");
    }
    if username.starts_with('-') || username.ends_with('-') {
        return Err("Username cannot start or end with a symbol");
    }
    if RESERVED_USERNAMES.contains(&username) {
        return Err("This username is reserved and cannot be used");
    }
    Ok(())
}

/// Mirrors `synaptic_core::models::user::validate_display_name`.
fn validate_display_name(display_name: &str) -> Result<(), &'static str> {
    if display_name.chars().count() > 60 {
        return Err("Display name must be no more than 60 characters");
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
