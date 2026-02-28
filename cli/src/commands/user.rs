use clap::Subcommand;
use dialoguer::{Input, Password, Select};
use uuid::Uuid;

#[derive(Subcommand)]
pub enum UserAction {
    /// Create a new user interactively
    Create,
    /// List all users
    List,
    /// Reset a user's password
    ResetPassword,
}

pub async fn run(action: UserAction) -> anyhow::Result<()> {
    match action {
        UserAction::Create => create().await,
        UserAction::List => list().await,
        UserAction::ResetPassword => reset_password().await,
    }
}

async fn create() -> anyhow::Result<()> {
    let pool = super::connect_db().await?;

    let username: String = Input::new()
        .with_prompt("Username")
        .interact_text()?;

    let email: String = Input::new()
        .with_prompt("Email")
        .interact_text()?;

    let display_name: String = Input::new()
        .with_prompt("Display name")
        .default(username.clone())
        .interact_text()?;

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

    // Hash password with Argon2
    let hash = hash_password(&password)?;

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, username, email, display_name, password_hash, role, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, NOW())"
    )
    .bind(id)
    .bind(&username)
    .bind(&email)
    .bind(&display_name)
    .bind(&hash)
    .bind(role)
    .execute(&pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create user: {e}"))?;

    println!("\nUser created successfully.");
    println!("  ID:       {}", id);
    println!("  Username: {}", username);
    println!("  Email:    {}", email);
    println!("  Role:     {}", role);

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

fn hash_password(password: &str) -> anyhow::Result<String> {
    use argon2::{password_hash::{rand_core::OsRng, PasswordHasher, SaltString}, Argon2};
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("Password hashing failed: {e}"))
}
