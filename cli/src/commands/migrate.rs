use clap::Args;

#[derive(Args)]
pub struct MigrateArgs {
    /// Database URL (overrides DATABASE_URL env var)
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: Option<String>,
}

pub async fn run(args: MigrateArgs) -> anyhow::Result<()> {
    if let Some(url) = args.database_url {
        std::env::set_var("DATABASE_URL", url);
    }

    let pool = super::connect_db().await?;

    println!("Running database migrations...");
    sqlx::migrate!("../migrations")
        .run(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Migration failed: {e}"))?;

    println!("Migrations applied successfully.");
    Ok(())
}
