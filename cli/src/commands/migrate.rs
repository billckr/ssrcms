use clap::Args;

#[derive(Args)]
pub struct MigrateArgs {
    /// Database URL (overrides DATABASE_URL env var)
    #[arg(long, env = "DATABASE_URL", hide = true)]
    pub database_url: Option<String>,
}

pub async fn run(args: MigrateArgs) -> anyhow::Result<()> {
    if let Some(url) = args.database_url {
        std::env::set_var("DATABASE_URL", url);
    }

    let pool = super::connect_db().await?;

    // Check for migrations the DB has applied that this binary doesn't know about.
    // This happens when the binary is older than the codebase.
    let migrator = sqlx::migrate!("../migrations");
    let known_versions: std::collections::HashSet<i64> =
        migrator.migrations.iter().map(|m| m.version).collect();

    let applied: Vec<i64> = sqlx::query_scalar(
        "SELECT version FROM _sqlx_migrations ORDER BY version"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let applied_set: std::collections::HashSet<i64> = applied.iter().copied().collect();

    let unknown: Vec<i64> = applied
        .into_iter()
        .filter(|v| !known_versions.contains(v))
        .collect();

    if !unknown.is_empty() {
        let list = unknown.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ");
        anyhow::bail!(
            "This CLI binary is outdated.\n\
             The database has migrations the binary doesn't know about: {list}\n\
             \n\
             To fix, reinstall the CLI from the project root:\n\
             \n\
             ./app.sh update-cli\n\
             \n\
             Then re-run: synap-cli migrate"
        );
    }

    // Determine which migrations are pending before running.
    let pending: Vec<_> = migrator.migrations.iter()
        .filter(|m| !applied_set.contains(&m.version))
        .collect();

    if pending.is_empty() {
        println!("Database is up to date — no migrations to run.");
        return Ok(());
    }

    println!("Applying {} migration{}:", pending.len(), if pending.len() == 1 { "" } else { "s" });
    for m in &pending {
        println!("  [{:04}] {}", m.version, m.description);
    }

    migrator
        .run(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Migration failed: {e}"))?;

    println!("Done.");
    Ok(())
}
